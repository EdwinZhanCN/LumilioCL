use super::Wiring;
use lumilio_ui::live::{UpdateStatus, UpdateUnavailability};
use lumilio_updater::{CheckResult, UpdateClient, UpdateError, UpdateRestriction};
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
    Restart,
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
                UpdateEvent::Restart => {
                    cx.update(|cx| cx.quit());
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
    let mut ready: Option<(String, PathBuf)> = None;
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
                        let Some((_, path)) = ready.as_ref() else { continue };
                        match client.install_and_restart(path) {
                            Ok(()) => {
                                let _ = events.send(UpdateEvent::Restart);
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
    previous: Option<(String, PathBuf)>,
) -> Option<(String, PathBuf)> {
    if let Some(ready) = check(client, events, manual).await {
        return Some(ready);
    }
    if let Some((version, _)) = &previous {
        publish(
            events,
            UpdateStatus::Ready {
                version: version.clone(),
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
) -> Option<(String, PathBuf)> {
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
    publish(
        events,
        UpdateStatus::Downloading {
            version: version.clone(),
        },
    );
    match client.download(&release).await {
        Ok(path) => {
            publish(
                events,
                UpdateStatus::Ready {
                    version: version.clone(),
                },
            );
            Some((version, path))
        }
        Err(error) => {
            report_failure(events, error, manual);
            None
        }
    }
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
