use super::buttons::{LocalActionIcon, art_button};
use super::launching::phase_label;
use super::render::{ArtButtons, caption, eyebrow, headline, world_accent};
use super::{HomeIntent, HomeIntentHandler, RecoveryDetail, Subject};
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, px};
use gpui_component::{h_flex, v_flex};
use lumilio_core::LaunchFailure;

/// One plain sentence about what went wrong (design language §8).
pub fn recovery_sentence(detail: &RecoveryDetail) -> String {
    let code = |code: &Option<i32>| {
        code.map(|code| format!("（退出代码 {code}）"))
            .unwrap_or_default()
    };
    match detail {
        RecoveryDetail::Interrupted => "上次没有正常结束。".to_owned(),
        RecoveryDetail::Failed {
            failure: LaunchFailure::ExitedEarly { code: exit },
            ..
        } => format!("游戏在启动时退出了{}。", code(exit)),
        RecoveryDetail::Failed { phase, .. } => {
            format!("在「{}」这一步停了下来。", phase_label(*phase))
        }
        RecoveryDetail::Crashed { code: exit } => format!("游戏意外退出了{}。", code(exit)),
    }
}

pub(super) fn render_recovery(
    subject: &Subject,
    detail: &RecoveryDetail,
    intent_handler: Option<HomeIntentHandler>,
    art: ArtButtons,
) -> AnyElement {
    let title = match detail {
        RecoveryDetail::Crashed { .. } => "游戏意外退出了",
        _ => "上次没有启动成功",
    };
    v_flex()
        .items_start()
        .gap(px(6.))
        .child(eyebrow("需要看一眼", world_accent(subject.world)))
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
                // ia[home]: 恢复并继续 | 启动失败后的「恢复并继续」 | 修复后重试启动 | —
                .child(art_button(
                    "home-recover",
                    "恢复并继续",
                    LocalActionIcon::Recover,
                    HomeIntent::Recover,
                    art.primary,
                    intent_handler.clone(),
                ))
                // ia[home]: 技术详情 | 启动失败后的「技术详情」 | 失败原因弹窗 | —
                .child(art_button(
                    "home-details",
                    "技术详情",
                    LocalActionIcon::TechnicalDetails,
                    HomeIntent::TechnicalDetails,
                    art.glass,
                    intent_handler,
                )),
        )
        .into_any_element()
}
