use super::Wiring;
use lumilio_ui::live::{UpdateStatus, UpdateUnavailability};
use lumilio_updater::{CheckResult, Relaunch, UpdateClient, UpdateError, UpdateRestriction};
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;

const UPDATE_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub(super) enum UpdateCommand {
    CheckManually,
    SetAutomatic(bool),
    Apply,
}

enum UpdateEvent {
    Status(UpdateStatus),
    Relaunch(Relaunch),
}

/// A verified download. Where the platform allows it, it is installed as
/// soon as it arrives, so the restart key only restarts.
#[derive(Clone)]
struct Ready {
    version: String,
    path: PathBuf,
    installed: Option<Relaunch>,
}

pub(super) fn start(wiring: &Wiring, cx: &mut gpui_kit::App) {
    let (commands, command_rx) = mpsc::unbounded_channel();
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    wiring.state.borrow_mut().update_commands = Some(commands);

    let service = wiring.backend.service.clone();
    let data_dir = service.layout().root().to_owned();
    let client = UpdateClient::from_build(data_dir);
    wiring.backend.spawn(async move {
        let auto_update = service.settings().await.auto_update;
        run(client, auto_update, command_rx, event_tx).await;
    });

    let shell = wiring.shell.clone();
    cx.spawn(async move |cx| {
        while let Some(event) = event_rx.recv().await {
            match event {
                UpdateEvent::Status(status) => {
                    let _ = shell.update(cx, |shell, cx| {
                        shell.update_live(|model| model.update_status = status, cx);
                    });
                }
                UpdateEvent::Relaunch(relaunch) => {
                    cx.update(|cx| match relaunch {
                        // GPUI waits for this process to exit before it
                        // opens the path again, so macOS reuses the Dock
                        // item instead of adding a second instance.
                        Relaunch::Restart(path) => {
                            cx.set_restart_path(path);
                            cx.restart();
                        }
                        Relaunch::Quit => cx.quit(),
                    });
                    break;
                }
            }
        }
    })
    .detach();
}

pub(super) fn check_manually(wiring: &Wiring) {
    send(wiring, UpdateCommand::CheckManually);
}

pub(super) fn set_automatic(wiring: &Wiring, enabled: bool) {
    send(wiring, UpdateCommand::SetAutomatic(enabled));
}

pub(super) fn apply(wiring: &Wiring) {
    // The navigation control is disabled for a running game. Keep the same
    // guard here for launch-in-progress and stale UI events.
    if wiring.state.borrow().cancel.is_some() {
        return;
    }
    send(wiring, UpdateCommand::Apply);
}

fn send(wiring: &Wiring, command: UpdateCommand) {
    if let Some(sender) = wiring.state.borrow().update_commands.as_ref() {
        let _ = sender.send(command);
    }
}

async fn run(
    client: UpdateClient,
    mut auto_update: bool,
    mut commands: mpsc::UnboundedReceiver<UpdateCommand>,
    events: mpsc::UnboundedSender<UpdateEvent>,
) {
    let mut ready: Option<Ready> = None;
    if let Some(reason) = client.restriction() {
        auto_update = false;
        publish(&events, UpdateStatus::Unavailable(view_restriction(reason)));
    } else if auto_update {
        ready = refresh(&client, &events, false, ready).await;
    }

    let mut interval = tokio::time::interval(UPDATE_INTERVAL);
    interval.tick().await;
    loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(UpdateCommand::CheckManually) => {
                        if let Some(reason) = client.restriction() {
                            publish(&events, UpdateStatus::Unavailable(view_restriction(reason)));
                        } else {
                            ready = refresh(&client, &events, true, ready).await;
                        }
                    }
                    Some(UpdateCommand::SetAutomatic(enabled)) => {
                        auto_update = enabled && client.restriction().is_none();
                        if auto_update {
                            ready = refresh(&client, &events, false, ready).await;
                        }
                    }
                    Some(UpdateCommand::Apply) => {
                        let Some(update) = ready.as_ref() else { continue };
                        let relaunch = match &update.installed {
                            Some(relaunch) => Ok(relaunch.clone()),
                            None => {
                                publish(&events, UpdateStatus::Installing {
                                    version: update.version.clone(),
                                });
                                install(&client, update.path.clone()).await
                            }
                        };
                        match relaunch {
                            Ok(relaunch) => {
                                let _ = events.send(UpdateEvent::Relaunch(relaunch));
                            }
                            Err(error) => publish(&events, failed(error)),
                        }
                    }
                    None => break,
                }
            }
            _ = interval.tick(), if auto_update => {
                ready = refresh(&client, &events, false, ready).await;
            }
        }
    }
}

async fn refresh(
    client: &UpdateClient,
    events: &mpsc::UnboundedSender<UpdateEvent>,
    manual: bool,
    previous: Option<Ready>,
) -> Option<Ready> {
    if let Some(ready) = check(client, events, manual, previous.as_ref()).await {
        return Some(ready);
    }
    if let Some(ready) = &previous {
        publish(
            events,
            UpdateStatus::Ready {
                version: ready.version.clone(),
            },
        );
    } else if !manual {
        publish(events, UpdateStatus::NotChecked);
    }
    previous
}

async fn check(
    client: &UpdateClient,
    events: &mpsc::UnboundedSender<UpdateEvent>,
    manual: bool,
    previous: Option<&Ready>,
) -> Option<Ready> {
    publish(events, UpdateStatus::Checking);
    let release = match client.check().await {
        Ok(CheckResult::UpToDate) => {
            publish(events, UpdateStatus::UpToDate);
            return None;
        }
        Ok(CheckResult::Available(release)) => release,
        Err(error) => {
            report_failure(events, error, manual);
            return None;
        }
    };
    let version = release.version().to_string();
    // The hourly check finds the same release until the launcher restarts;
    // it is already downloaded and, where possible, installed.
    if let Some(ready) = previous.filter(|ready| ready.version == version) {
        publish(events, UpdateStatus::Ready { version });
        return Some(ready.clone());
    }
    publish(
        events,
        UpdateStatus::Downloading {
            version: version.clone(),
        },
    );
    let path = match client.download(&release).await {
        Ok(path) => path,
        Err(error) => {
            report_failure(events, error, manual);
            return None;
        }
    };
    let installed = if client.installs_while_running() {
        publish(
            events,
            UpdateStatus::Installing {
                version: version.clone(),
            },
        );
        match install(client, path.clone()).await {
            Ok(relaunch) => Some(relaunch),
            Err(error) => {
                report_failure(events, error, manual);
                return None;
            }
        }
    } else {
        None
    };
    publish(
        events,
        UpdateStatus::Ready {
            version: version.clone(),
        },
    );
    Some(Ready {
        version,
        path,
        installed,
    })
}

/// Installation runs external tools that block; keep them off the runtime's
/// worker threads.
async fn install(client: &UpdateClient, path: PathBuf) -> Result<Relaunch, UpdateError> {
    let client = client.clone();
    tokio::task::spawn_blocking(move || client.install(&path))
        .await
        .unwrap_or_else(|error| Err(UpdateError::Install(error.to_string())))
}

fn failed(error: UpdateError) -> UpdateStatus {
    UpdateStatus::Failed {
        detail: error.to_string(),
    }
}

fn report_failure(events: &mpsc::UnboundedSender<UpdateEvent>, error: UpdateError, manual: bool) {
    if manual {
        publish(events, failed(error));
    } else {
        eprintln!("LumilioCL automatic update check failed: {error}");
    }
}

fn publish(events: &mpsc::UnboundedSender<UpdateEvent>, status: UpdateStatus) {
    let _ = events.send(UpdateEvent::Status(status));
}

fn view_restriction(reason: UpdateRestriction) -> UpdateUnavailability {
    match reason {
        UpdateRestriction::DebugBuild => UpdateUnavailability::DebugBuild,
        UpdateRestriction::PortableWindows => UpdateUnavailability::PortableWindows,
        UpdateRestriction::PackageManagedLinux => UpdateUnavailability::PackageManagedLinux,
        UpdateRestriction::UnsupportedInstall => UpdateUnavailability::UnsupportedInstall,
    }
}
