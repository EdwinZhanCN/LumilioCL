use super::editors::Editor;
use super::intent::{InstanceIntent, Section};
use super::{
    InstanceDetailView, TAB_CONTENT, TAB_DIAGNOSTICS, TAB_HISTORY, TAB_OVERVIEW, TAB_WORLDS, TABS,
};
use crate::theme::ShellColors;
use crate::{kit, live, theme};
use gpui::prelude::*;
use gpui::{Context, IntoElement, Render, Window, div, px};
use gpui_component::ActiveTheme as _;
use gpui_component::{TITLE_BAR_HEIGHT, h_flex, v_flex};

impl Render for InstanceDetailView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_fields(window, cx);
        self.settle_editor(window, cx);
        crate::toast::flush(&mut self.toasts, window, cx);
        if self.record.is_some()
            && let Some(question) = self.ask_later.take()
        {
            match question {
                InstanceIntent::AskCopy => self.open_editor(Editor::Copy, window, cx),
                InstanceIntent::AskDelete => self.confirm_delete(window, cx),
                InstanceIntent::Resolve(action) => self.run_problem(action, window, cx),
                _ => {}
            }
        }
        if self.inspect.is_some() {
            self.open_inspected_crash(window, cx);
        }
        if std::mem::take(&mut self.refresh_worlds) && self.data.has(Section::Worlds) {
            self.request(Section::Worlds, window, cx);
        }
        if std::mem::take(&mut self.ping_servers) && self.tab == TAB_WORLDS && self.worlds_sub == 1
        {
            self.ping_all(window, cx);
        }
        if std::mem::take(&mut self.refresh_logs) && self.data.has(Section::Logs) {
            self.request(Section::Logs, window, cx);
        }
        if self.record.is_some() && !self.started {
            self.started = true;
            self.ensure(Section::Problems, window, cx);
            self.ensure(Section::Size, window, cx);
            self.ensure(Section::History, window, cx);
        }
        let colors = ShellColors::from_theme(cx.theme());
        let content = if self.record.is_some() {
            match self.tab {
                TAB_OVERVIEW => self.overview(colors, cx),
                TAB_CONTENT => self.content_panel(colors, cx),
                TAB_WORLDS => self.worlds_panel(colors, cx),
                TAB_HISTORY => self.history_panel(colors, cx),
                TAB_DIAGNOSTICS => self.diagnostics_panel(colors, cx),
                _ => self.settings_tab(colors, cx),
            }
        } else if let Some(message) = &self.error {
            let retry = self.handler.clone();
            v_flex()
                .gap_3()
                .child(kit::empty("没有读到游戏", message.clone(), colors))
                .child(
                    h_flex()
                        .justify_center()
                        .gap_2()
                        .child(kit::action(
                            "instance-retry",
                            "重试",
                            None,
                            true,
                            move |window, cx| retry(InstanceIntent::Reload, window, cx),
                        ))
                        .children(
                            self.load_technical
                                .clone()
                                .map(|detail| kit::technical("instance-technical", detail)),
                        ),
                )
                .into_any_element()
        } else {
            kit::empty("正在读取游戏…", "", colors).into_any_element()
        };
        let tabs = cx.listener(|view, index: &usize, window, cx| view.open_tab(*index, window, cx));
        let content = v_flex()
            .w_full()
            .gap_5()
            // Room for the cover above the title; going back is the
            // navigation's job (design language §6).
            .child(div().h(px(84.)))
            .child(kit::header(
                self.title().to_owned(),
                self.record
                    .as_ref()
                    .map_or_else(String::new, live::instance_meta),
                self.page_actions(colors, cx),
                colors,
            ))
            .child(kit::tabs(
                "live-instance-tabs",
                &TABS,
                self.tab,
                move |index, window, cx| tabs(&index, window, cx),
            ))
            .child(kit::entrance(content, ("live-instance-body", self.tab)));
        div()
            .id("live-instance")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .relative()
                    .w_full()
                    .children(self.record.as_ref().map(|record| {
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .h(px(168.))
                            .child(crate::cover::element(
                                live::seed_of(&record.id),
                                live::cover_loader(record.loader),
                                live::world_of(&record.id),
                                colors.background,
                                theme::HERO_FADE,
                                px(0.),
                            ))
                    }))
                    .child(
                        theme::content_column()
                            .mx_auto()
                            .pt(TITLE_BAR_HEIGHT + px(12.))
                            .pb(theme::BOTTOM_SAFE_AREA)
                            .child(content),
                    ),
            )
    }
}
