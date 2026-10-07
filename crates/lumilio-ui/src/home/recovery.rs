use super::buttons::{LocalActionIcon, art_button};
use super::launching::phase_label;
use super::render::{ArtButtons, caption, eyebrow, headline, world_accent};
use super::{HomeIntent, HomeIntentHandler, RecoveryDetail, Subject};
use crate::tr;
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, px};
use gpui_component::{h_flex, v_flex};
use lumilio_core::LaunchFailure;

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
        RecoveryDetail::Failed { phase, .. } => {
            tr!("home-recovery-stopped-at", phase = phase_label(*phase))
        }
        RecoveryDetail::Crashed { code: Some(code) } => {
            tr!("home-recovery-crashed-code", code = code.to_string())
        }
        RecoveryDetail::Crashed { code: None } => tr!("home-recovery-crashed").to_owned(),
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
            tr!("home-recovery-eyebrow"),
            world_accent(subject.world),
        ))
        .child(headline(title))
        .child(caption(format!(
            "{} · {}",
            subject.title,
            recovery_sentence(detail)
        )))
        .child(
            h_flex()
                .mt(px(12.))
                .gap_3()
                // ia[home]: 恢复并继续 | 启动失败后的「恢复并继续」 | 修复后重试启动
                .child(art_button(
                    "home-recover",
                    tr!("home-recover"),
                    LocalActionIcon::Recover,
                    HomeIntent::Recover,
                    art.primary,
                    intent_handler.clone(),
                ))
                // ia[home]: 技术详情 | 启动失败后的「技术详情」 | 失败原因弹窗
                .child(art_button(
                    "home-details",
                    tr!("common-technical-details"),
                    LocalActionIcon::TechnicalDetails,
                    HomeIntent::TechnicalDetails,
                    art.glass,
                    intent_handler,
                )),
        )
        .into_any_element()
}
