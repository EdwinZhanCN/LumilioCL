use super::buttons::{LocalActionIcon, art_button};
use super::launching::phase_label;
use super::render::{ArtButtons, caption, eyebrow, headline, world_accent};
use super::{HomeIntent, HomeIntentHandler, RecoveryDetail, Subject};
use crate::tr;
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, px};
use gpui_component::{h_flex, v_flex};
use lumilio_core::{LaunchFailure, LaunchProblem};

/// One plain sentence about what went wrong (design language §8).
pub fn recovery_sentence(detail: &RecoveryDetail) -> String {
    // An exit code is an identifier, not a quantity: no digit grouping.
    match detail {
        RecoveryDetail::Interrupted => tr!("home-recovery-interrupted").to_owned(),
        RecoveryDetail::Failed {
            failure: LaunchFailure::ExitedEarly { code: Some(code) },
            ..
        } => tr!("home-recovery-exited-early-code", code = code.to_string()),
        RecoveryDetail::Failed {
            failure: LaunchFailure::ExitedEarly { code: None },
            ..
        } => tr!("home-recovery-exited-early").to_owned(),
        // The cause, when the launcher knows one the person can act on;
        // otherwise the step it stopped at.
        RecoveryDetail::Failed {
            failure:
                LaunchFailure::Step {
                    problem: Some(problem),
                    ..
                },
            ..
        } => problem_sentence(problem),
        RecoveryDetail::Failed { phase, .. } => {
            tr!("home-recovery-stopped-at", phase = phase_label(*phase))
        }
        RecoveryDetail::Crashed { code: Some(code) } => {
            tr!("home-recovery-crashed-code", code = code.to_string())
        }
        RecoveryDetail::Crashed { code: None } => tr!("home-recovery-crashed").to_owned(),
    }
}

/// The line under Recovery's headline. A crash's headline already says the
/// game quit, so the line only adds the exit code.
pub(super) fn recovery_caption(subject: &Subject, detail: &RecoveryDetail) -> String {
    match detail {
        RecoveryDetail::Crashed { code: Some(code) } => format!(
            "{} · {}",
            subject.title,
            tr!("home-recovery-exit-code", code = code.to_string())
        ),
        RecoveryDetail::Crashed { code: None } => subject.title.clone(),
        _ => format!("{} · {}", subject.title, recovery_sentence(detail)),
    }
}

fn problem_sentence(problem: &LaunchProblem) -> String {
    // Exit codes and Java majors are identifiers: no digit grouping.
    match problem {
        LaunchProblem::SignInRequired(name) => tr!(
            "home-recovery-reason",
            reason = tr!("account-failure-sign-in-required", name = name.as_str())
        ),
        LaunchProblem::Account(error) => tr!(
            "home-recovery-reason",
            reason = crate::live::auth_message(error)
        ),
        LaunchProblem::NoJava(Some(major)) => {
            tr!("home-recovery-no-java-major", major = major.to_string())
        }
        LaunchProblem::NoJava(None) => tr!("home-recovery-no-java").to_owned(),
        LaunchProblem::JavaDownload => tr!("home-recovery-java-download").to_owned(),
        LaunchProblem::BeforeLaunch(Some(code)) => {
            tr!("home-recovery-before-launch-code", code = code.to_string())
        }
        LaunchProblem::BeforeLaunch(None) => tr!("home-recovery-before-launch").to_owned(),
        LaunchProblem::WorldMissing(world) => {
            tr!("home-recovery-world-missing", world = world.as_str())
        }
    }
}

pub(super) fn render_recovery(
    subject: &Subject,
    detail: &RecoveryDetail,
    intent_handler: Option<HomeIntentHandler>,
    art: ArtButtons,
) -> AnyElement {
    let title = match detail {
        RecoveryDetail::Crashed { .. } => tr!("home-recovery-crashed-title"),
        _ => tr!("home-recovery-failed-title"),
    };
    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow(
            art,
            tr!("home-recovery-eyebrow"),
            world_accent(subject.world),
        ))
        .child(headline(art, title))
        .child(caption(art, recovery_caption(subject, detail)))
        .child(
            h_flex()
                .mt(px(12.))
                .gap_3()
                // ia[home]: 恢复并继续 | 启动失败后的「恢复并继续」 | 重试失败的那个游戏；恢复界面只在它是当前游戏时出现，换了当前游戏后失败留在「需要留意」 | global-context T6
                .child(art_button(
                    "home-recover",
                    tr!("home-recover"),
                    LocalActionIcon::Recover,
                    HomeIntent::Recover,
                    art.primary,
                    intent_handler.clone(),
                ))
                // ia[home]: 技术详情 | 启动失败后的「技术详情」 | 准备步骤失败时弹出失败原因，可复制；游戏崩溃、启动时退出或上次未正常结束时打开该游戏的诊断页
                .child(
                    art_button(
                        "home-details",
                        tr!("common-technical-details"),
                        LocalActionIcon::TechnicalDetails,
                        HomeIntent::TechnicalDetails,
                        art.glass,
                        intent_handler,
                    )
                    .debug_selector(|| "home-details".into()),
                ),
        )
        .into_any_element()
}
