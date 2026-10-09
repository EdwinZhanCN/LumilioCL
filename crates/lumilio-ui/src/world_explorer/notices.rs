//! The messages menu in the map's bottom-left corner: what is wrong or
//! missing, the key that retries it, and the offer to link Xaero data. The
//! map shows only a dot when there is something to read (design language §11).
use super::MapView;
use crate::assets::UiIcon;
use crate::{key::Key, theme::ShellColors, tr};
use gpui::{Anchor, AnyElement, Context, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, Icon, Sizable as _, popover::Popover, v_flex};

impl MapView {
    /// Anything for the messages menu to show.
    fn has_notices(&self) -> bool {
        self.problem_line().is_some()
            || self.failed.len() + self.objects.failed() > 0
            || self.xaero_suggestion().is_some()
    }

    /// The messages key with a dot on it while the menu has something to read.
    pub(super) fn notice_popover(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = ShellColors::from_theme(cx.theme());
        let target = cx.weak_entity();
        // ia[plugin.world-explorer]: 查看地图消息 | 地图左下角 ·「消息」键与小红点 | 有失败、缺少的输入或可关联的 Xaero 数据时键上亮一个点（不带数字）；点开列出消息、重试键与关联键；Escape 或点击外部关闭
        let popover = Popover::new("map-notice-popover")
            .anchor(Anchor::BottomLeft)
            .trigger(
                Key::new("map-notices")
                    .icon(Icon::new(UiIcon::Info))
                    .white()
                    .small()
                    .tooltip(tr!("map-notices"))
                    .debug_selector(|| "map-notices".into()),
            )
            .content(move |_, _, cx| {
                target
                    .update(cx, |this, cx| this.notice_list(cx))
                    .unwrap_or_else(|_| div().into_any_element())
            });
        div()
            .relative()
            .child(popover)
            .when(self.has_notices(), |wrap| {
                wrap.child(
                    div()
                        .absolute()
                        .top(px(-2.))
                        .right(px(-2.))
                        .size(px(8.))
                        .rounded_full()
                        .bg(colors.body.orange_text)
                        .debug_selector(|| "map-notice-dot".into()),
                )
            })
            .into_any_element()
    }

    fn notice_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let problem = self.problem_line();
        let retry = self.retry_button(cx);
        let link = self.xaero_link_prompt(cx);
        let empty = problem.is_none() && retry.is_none() && link.is_none();
        v_flex()
            .w(px(280.))
            .gap_2()
            .text_xs()
            .debug_selector(|| "map-notice-panel".into())
            .children(problem.map(|text| div().debug_selector(|| "map-status".into()).child(text)))
            .children(retry)
            .children(link)
            .when(empty, |list| {
                list.child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(tr!("map-notices-empty")),
                )
            })
            .into_any_element()
    }
}
