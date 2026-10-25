use gpui_kit::{App, AppContext as _};
use lumilio_core::{LaunchFailure, LaunchSignal, ServiceError};
use lumilio_ui::home::{HomePresentation, Subject};

use super::Wiring;

/// Dev hook: `LUMILIO_HOME_RECOVERY=failed|details|crashed|crashed-details|exited|exited-details|away`
/// puts the current game through a launch that goes wrong, by the same
/// signals a real one sends, so Recovery can be reviewed without starting a
/// game: `failed` stops at a step (the account must sign in again),
/// `crashed` quits while playing without an exit code, `exited` quits while
/// starting after printing a Java error, `away` fails while Library is open,
/// and the `details` cases also press 技术详情.
pub(super) fn review_recovery(
    wiring: &Wiring,
    which: String,
    window: gpui_kit::AnyWindowHandle,
    cx: &mut App,
) {
    // Home explains a failure only for the current game (D8).
    let current = wiring.backend.service.context().current_instance;
    let service = wiring.backend.service.clone();
    let handle = wiring
        .backend
        .spawn(async move { service.library().await.instances });
    let wiring = wiring.clone();
    cx.spawn(async move |cx| {
        let Some(records) = lumilio_ui::runtime::answer(handle, cx).await else {
            return;
        };
        let cards = lumilio_ui::live::library_cards(&records, lumilio_core::unix_now());
        let Some(card) = cards
            .into_iter()
            .find(|card| Some(&card.id) == current.as_ref())
        else {
            return;
        };
        let signals = match which.as_str() {
            "crashed" | "crashed-details" => {
                vec![LaunchSignal::Running, LaunchSignal::Exited { code: None }]
            }
            "exited" | "exited-details" => vec![
                LaunchSignal::Phase(lumilio_core::LaunchPhase::Starting),
                LaunchSignal::Failed(LaunchFailure::ExitedEarly {
                    code: Some(1),
                    output: [
                        "Exception in thread \"main\" java.lang.IllegalStateException: Module named org.objectweb.asm was already on the JVMs module path loaded from /launcher/meta/libraries/org/ow2/asm/asm/9.8/asm-9.8.jar but class-path contains it at location /launcher/meta/libraries/org/ow2/asm/asm/9.6/asm-9.6.jar",
                        "\tat cpw.mods.bootstraplauncher@2.0.2/cpw.mods.bootstraplauncher.BootstrapLauncher.run(BootstrapLauncher.java:139)",
                        "\tat cpw.mods.bootstraplauncher@2.0.2/cpw.mods.bootstraplauncher.BootstrapLauncher.main(BootstrapLauncher.java:69)",
                    ]
                    .map(str::to_owned)
                    .to_vec(),
                }),
            ],
            _ => vec![
                LaunchSignal::Phase(lumilio_core::LaunchPhase::Verifying),
                LaunchSignal::Failed(ServiceError::SignInRequired("Steve".into()).launch_failure()),
            ],
        };
        let _ = wiring.shell.update(cx, |shell, cx| {
            if which == "away" {
                shell.show(lumilio_ui::route::Route::Library);
            }
            shell.set_home(
                HomePresentation::Continue {
                    subject: Subject {
                        id: Some(card.id),
                        title: card.name,
                        metadata: format!("{} · {}", card.meta, card.played),
                        world: card.world,
                    },
                }
                .begin_launch(),
                cx,
            );
            for signal in signals {
                shell.apply_launch_signal(signal, cx);
            }
        });
        if which.ends_with("details") {
            let _ = cx.update_window(window, |_, window, cx| {
                let _ = wiring
                    .shell
                    .update(cx, |shell, cx| shell.technical_details(window, cx));
            });
        }
    })
    .detach();
}
