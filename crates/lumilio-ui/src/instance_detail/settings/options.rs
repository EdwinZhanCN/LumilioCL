//! 设置 › 选项: the game's own options (what its in-game Options screen
//! holds), edited while the game is closed. On top, where the options come
//! from (a preset or the game itself) and anything that waits for the player;
//! then the options in the game's own groups.
use super::super::{InstanceDetailView, InstanceIntent, Section};
use crate::game_settings::{self, OnEdit, OnToggle, ReviewActions};
use crate::theme::ShellColors;
use crate::{kit, tr};
use gpui::prelude::*;
use gpui::{AnyElement, Context, div};
use gpui_component::{h_flex, v_flex};
use lumilio_core::{GameSettingValue, InstanceOptions, OptionChange, OptionsFile};
use std::rc::Rc;

impl InstanceDetailView {
    pub(super) fn group_options(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let options = match &self.data.game_options {
            None => {
                return div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child(tr!("options-loading"))
                    .into_any_element();
            }
            Some(Err(detail)) => {
                let handler = self.handler.clone();
                return v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(tr!("options-load-failed")),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(kit::action(
                                "options-retry",
                                tr!("common-retry"),
                                None,
                                false,
                                move |window, cx| {
                                    handler(InstanceIntent::Load(Section::GameOptions), window, cx)
                                },
                            ))
                            .child(kit::technical("options-technical", detail.clone())),
                    )
                    .into_any_element();
            }
            Some(Ok(options)) => options,
        };
        // A running game owns its file; a write in progress owns the page.
        let running = options.view.is_busy || self.live_output.is_some();
        let still = running || self.busy;
        v_flex()
            .w_full()
            .gap_6()
            .children(running.then(|| {
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .debug_selector(|| "options-running".into())
                    .child(tr!("options-busy"))
            }))
            .child(self.options_source(options, still, colors))
            .children(self.options_notices(options, still, colors, cx))
            .children(self.options_review(options, still, colors, cx))
            .child(self.options_table(options, still, colors, cx))
            .into_any_element()
    }

    /// Which preset the game follows, with the way to change it.
    fn options_source(
        &self,
        options: &InstanceOptions,
        still: bool,
        colors: ShellColors,
    ) -> AnyElement {
        let value = match &options.view.preset {
            Some(preset) => preset.name.clone(),
            None => tr!("options-preset-none").to_owned(),
        };
        let handler = self.handler.clone();
        // ia[instance.settings]: 选择预设 | 设置 › 选项，「预设」行的「更换…」→ 弹窗：不使用预设或某个预设，并预览下次启动会改变的选项 | 选了预设后下次启动应用它共享的选项；改回不使用时保留现在的选项；游戏运行、有待决定的修改或上次会话未确认关闭时不能换
        let change = crate::theme::clickable(
            kit::ghost(
                "options-preset-change",
                tr!("options-preset-change"),
                move |window, cx| handler(InstanceIntent::ChoosePreset, window, cx),
            )
            .disabled(still)
            .debug_selector(|| "options-preset-change".into()),
            !still,
        );
        kit::list(
            vec![kit::value_row(
                "options-preset",
                tr!("options-preset"),
                Some(tr!("options-preset-help").into()),
                value,
                Some(change.into_any_element()),
                colors,
            )],
            colors,
        )
        .into_any_element()
    }

    /// Recovery, a failed sync and options the preset could not set.
    fn options_notices(
        &self,
        options: &InstanceOptions,
        still: bool,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let view = &options.view;
        let mut notices = Vec::new();
        if view.recovery_required {
            let weak = cx.entity().downgrade();
            // ia[instance.settings]: 确认游戏已关闭 | 设置 › 选项，上次会话中断时的说明和「游戏已关闭」 | 玩家确认后才比较游戏文件和上次应用的值，找出游戏里改过的选项；游戏还开着时不要点
            notices.push(
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(tr!("options-recovery-help")),
                    )
                    .child(
                        kit::action(
                            "options-recover",
                            tr!("options-recover"),
                            None,
                            false,
                            move |window, cx| {
                                let _ = weak.update(cx, |page, cx| {
                                    page.send(InstanceIntent::RecoverOptions, window, cx)
                                });
                            },
                        )
                        .disabled(still)
                        .debug_selector(|| "options-recover".into()),
                    )
                    .into_any_element(),
            );
        } else if let Some(issue) = &view.issue {
            notices.push(
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(colors.danger)
                            .child(tr!("options-sync-failed")),
                    )
                    .child(kit::technical("options-issue", issue.clone()))
                    .into_any_element(),
            );
        }
        if !view.skipped.is_empty() {
            notices.push(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.muted)
                            .child(tr!("options-skipped", count = view.skipped.len())),
                    )
                    .child(kit::info(
                        "options-skipped-info",
                        tr!("options-skipped-help"),
                    ))
                    .into_any_element(),
            );
        }
        notices
    }

    fn options_review(
        &self,
        options: &InstanceOptions,
        still: bool,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let view = options.view.clone();
        let weak = cx.entity().downgrade();
        let actions = ReviewActions {
            toggle: {
                let weak = weak.clone();
                Rc::new(move |key, _, cx| {
                    let _ = weak.update(cx, |page, cx| {
                        if !page.options_selected.remove(&key) {
                            page.options_selected.insert(key);
                        }
                        page.options_conflicts.clear();
                        cx.notify();
                    });
                })
            },
            all: {
                let (weak, keys) = (
                    weak.clone(),
                    view.pending
                        .iter()
                        .map(|change| change.setting.clone())
                        .collect::<Vec<_>>(),
                );
                Rc::new(move |every, _, cx| {
                    let _ = weak.update(cx, |page, cx| {
                        page.options_selected.clear();
                        if every {
                            page.options_selected.extend(keys.iter().cloned());
                        }
                        page.options_conflicts.clear();
                        cx.notify();
                    });
                })
            },
            resolve: {
                let expected = view.clone();
                Rc::new(move |resolution, window, cx| {
                    let expected = expected.clone();
                    let _ = weak.update(cx, |page, cx| {
                        let selected: Vec<String> = page.options_selected.iter().cloned().collect();
                        if !selected.is_empty() {
                            page.send(
                                InstanceIntent::ResolveOptions {
                                    expected,
                                    selected,
                                    resolution,
                                },
                                window,
                                cx,
                            );
                        }
                    });
                })
            },
        };
        game_settings::review(
            &view,
            &self.options_selected,
            &self.options_conflicts,
            still || view.recovery_required,
            colors,
            &actions,
        )
    }

    fn options_table(
        &self,
        options: &InstanceOptions,
        still: bool,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match &options.file {
            OptionsFile::Ready => {}
            OptionsFile::Missing => {
                return kit::empty(tr!("options-missing"), tr!("options-missing-help"), colors)
                    .into_any_element();
            }
            OptionsFile::Unsupported => {
                return kit::empty(
                    tr!(
                        "options-unsupported",
                        version = options.game_version.as_str()
                    ),
                    tr!("options-unsupported-help"),
                    colors,
                )
                .into_any_element();
            }
            OptionsFile::Unreadable(detail) => {
                return v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.foreground)
                            .child(tr!("options-unreadable")),
                    )
                    .child(
                        h_flex().child(kit::technical("options-unreadable-detail", detail.clone())),
                    )
                    .into_any_element();
            }
        }
        let lines = game_settings::instance_lines(options);
        if lines.is_empty() {
            return kit::empty(tr!("options-none"), "", colors).into_any_element();
        }
        let disabled = still || options.view.recovery_required;
        let on_edit: OnEdit = {
            let handler = self.handler.clone();
            Rc::new(move |key, window, cx| handler(InstanceIntent::EditOption(key), window, cx))
        };
        let on_toggle: OnToggle = {
            let (weak, expected) = (cx.entity().downgrade(), options.view.clone());
            Rc::new(move |key, on, window, cx| {
                let intent = InstanceIntent::SetOption {
                    expected: expected.clone(),
                    key,
                    change: OptionChange::Set(GameSettingValue::Boolean(on)),
                };
                let _ = weak.update(cx, |page, cx| page.send(intent, window, cx));
            })
        };
        // ia[instance.settings]: 修改选项 | 设置 › 选项的分组表：开关当场切换，其他选项「编辑」→ 弹窗（滑块和数值 / 分段 / 可搜索列表） | 游戏自己的选项当场写进 options.txt；预设共享的选项改成“仅此游戏”，下次启动生效，弹窗里可「跟随预设」改回；游戏运行时不能改
        game_settings::table("options", &lines, disabled, colors, &on_edit, &on_toggle)
    }
}
