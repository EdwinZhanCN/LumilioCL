//! 截图: the pictures the game took, as a wall of thumbnails.
//!
//! Thumbnails are made off the interface thread (the application asks core
//! for them); a card shows an empty tile until its thumbnail is there. Only the
//! cards on screen are asked for, a page at a time.

use super::super::{InstanceDetailView, InstanceIntent, Section};
use super::data::{Confirm, Thumb};
use super::helpers::{act, clock};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::kit;
use crate::theme::{self, ShellColors};
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, ObjectFit, Window, div, img, px};
use gpui_component::dialog::Dialog;
use gpui_component::{ActiveTheme as _, Icon, WindowExt as _, h_flex, v_flex};
use lumilio_core::ScreenshotInfo;

/// How many cards are shown at first, and added each time more are asked for.
pub const SHOTS_PAGE: usize = 48;
const CARD_WIDTH: f32 = 220.;
const CARD_HEIGHT: f32 = 124.;

impl InstanceDetailView {
    pub(in super::super) fn screenshots_panel(
        &self,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(status) = self.status(&self.data.screenshots, colors, Section::Screenshots, cx)
        {
            return status;
        }
        let Some(Ok(shots)) = &self.data.screenshots else {
            return div().into_any_element();
        };
        // ia[instance.screenshots]: 刷新截图 | L2 次要「刷新」 | 重新读取 screenshots 文件夹；游戏里新拍的在游戏结束后也会自动出现
        let refresh = kit::action(
            "shots-refresh",
            "刷新",
            Some(UiIcon::Refresh),
            false,
            act(cx, |view, window, cx| {
                view.request(Section::Screenshots, window, cx)
            }),
        )
        .debug_selector(|| "shots-refresh".into());
        // ia[instance.screenshots]: 打开截图文件夹 | L2 次要「打开文件夹」 | 在访达中显示 screenshots 文件夹
        let folder = kit::action(
            "shots-folder",
            "打开文件夹",
            Some(UiIcon::External),
            false,
            act(cx, |view, window, cx| {
                (view.handler)(InstanceIntent::RevealPath("screenshots".into()), window, cx)
            }),
        )
        .debug_selector(|| "shots-folder".into());
        let controls = h_flex()
            .w_full()
            .justify_end()
            .gap_2()
            .child(refresh)
            .child(folder);
        if shots.is_empty() {
            return v_flex()
                .w_full()
                .gap_3()
                .child(controls)
                .child(kit::empty(
                    "还没有截图",
                    "在游戏里按 F2 截图，拍下的画面会出现在这里",
                    colors,
                ))
                .into_any_element();
        }
        let shown = self.shots_shown.min(shots.len());
        let cards = shots.iter().take(shown).enumerate().map(|(index, shot)| {
            let picture = match self.thumbs.get(&shot.file) {
                Some(Thumb::Ready(_, path)) => img(path.clone())
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .into_any_element(),
                Some(Thumb::Failed(_)) => div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(colors.muted)
                    .child("无法预览")
                    .into_any_element(),
                _ => div().size_full().into_any_element(),
            };
            let file = shot.file.clone();
            // ia[instance.screenshots]: 查看截图 | 缩略图卡片 → 大图弹窗 | 看大图，可前后翻、复制图片、在访达中显示、删除（先确认）
            v_flex()
                .id(("shot", index))
                .debug_selector(move || format!("shot-{index}"))
                .w(px(CARD_WIDTH))
                .gap_1()
                .cursor_pointer()
                .on_click(cx.listener(move |view, _, window, cx| {
                    view.open_screenshot(file.clone(), window, cx)
                }))
                .child(
                    div()
                        .w_full()
                        .h(px(CARD_HEIGHT))
                        .rounded(px(10.))
                        .overflow_hidden()
                        .bg(colors.surface_subtle)
                        .child(picture),
                )
                .child(div().text_xs().text_color(colors.muted).child(taken(shot)))
        });
        let more = (shown < shots.len()).then(|| {
            h_flex().w_full().justify_center().child(
                kit::action(
                    "shots-more",
                    "显示更多",
                    None,
                    false,
                    act(cx, |view, _, cx| view.show_more_screenshots(cx)),
                )
                .debug_selector(|| "shots-more".into()),
            )
        });
        v_flex()
            .w_full()
            .gap_3()
            .child(controls)
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .gap_3()
                    .debug_selector(|| "shots-wall".into())
                    .children(cards),
            )
            .children(more)
            .into_any_element()
    }

    /// Adds another page of cards and asks for their thumbnails.
    pub(in super::super) fn show_more_screenshots(&mut self, cx: &mut Context<Self>) {
        self.shots_shown += SHOTS_PAGE;
        self.ask_thumbs = true;
        cx.notify();
    }

    /// Asks for the thumbnails of the cards shown that are not there yet.
    pub(in super::super) fn ask_thumbnails(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Ok(shots)) = &self.data.screenshots else {
            return;
        };
        let wanted: Vec<_> = shots
            .iter()
            .take(self.shots_shown)
            .filter(|shot| {
                self.thumbs
                    .get(&shot.file)
                    .is_none_or(|thumb| thumb.modified_ms() != shot.modified_ms)
            })
            .map(|shot| (shot.file.clone(), shot.modified_ms))
            .collect();
        for (file, modified) in wanted {
            self.thumbs.insert(file.clone(), Thumb::Pending(modified));
            (self.handler)(InstanceIntent::Thumbnail(file), window, cx);
        }
    }

    /// A thumbnail was made (or could not be). An answer for a picture that
    /// has changed since it was asked for is dropped.
    pub fn thumbnail_arrived(
        &mut self,
        file: String,
        result: Result<std::path::PathBuf, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(Thumb::Pending(modified)) = self.thumbs.get(&file).cloned() else {
            return;
        };
        self.thumbs.insert(
            file,
            match result {
                Ok(path) => Thumb::Ready(modified, path),
                Err(_) => Thumb::Failed(modified),
            },
        );
        cx.notify();
    }

    /// The list arrived: forget thumbnails of pictures that are gone or changed.
    pub(in super::super) fn reconcile_thumbs(&mut self) {
        let Some(Ok(shots)) = &self.data.screenshots else {
            return;
        };
        self.thumbs.retain(|file, thumb| {
            shots
                .iter()
                .any(|shot| &shot.file == file && shot.modified_ms == thumb.modified_ms())
        });
        self.ask_thumbs = true;
    }

    fn open_screenshot(&mut self, file: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.shot_open.is_some() {
            return;
        }
        self.shot_open = Some(file);
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| match view.upgrade() {
            Some(view) => InstanceDetailView::shot_dialog(&view, dialog, cx),
            None => dialog,
        });
        cx.notify();
    }

    /// Moves the viewer to the neighbouring picture (wrapping around).
    fn step_screenshot(&mut self, delta: isize, cx: &mut Context<Self>) {
        let (Some(Ok(shots)), Some(file)) = (&self.data.screenshots, &self.shot_open) else {
            return;
        };
        let Some(at) = shots.iter().position(|shot| &shot.file == file) else {
            return;
        };
        let total = shots.len() as isize;
        let next = (at as isize + delta).rem_euclid(total) as usize;
        self.shot_open = Some(shots[next].file.clone());
        cx.notify();
    }

    fn shot_dialog(view: &Entity<Self>, dialog: Dialog, cx: &mut App) -> Dialog {
        let this = view.read(cx);
        let (Some(Ok(shots)), Some(file)) = (&this.data.screenshots, &this.shot_open) else {
            return dialog;
        };
        let Some(at) = shots.iter().position(|shot| &shot.file == file) else {
            return dialog;
        };
        let shot: &ScreenshotInfo = &shots[at];
        let (file, path, total) = (shot.file.clone(), shot.path.clone(), shots.len());
        let colors = theme::ShellColors::from_theme(cx.theme());
        let on = |run: fn(
            &mut InstanceDetailView,
            &str,
            &mut Window,
            &mut Context<InstanceDetailView>,
        )| {
            let (view, file) = (view.downgrade(), file.clone());
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                let _ = view.update(cx, |view, cx| run(view, &file, window, cx));
            }
        };
        let step = |delta: isize| {
            let view = view.downgrade();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                let _ = view.update(cx, |view, cx| view.step_screenshot(delta, cx));
            }
        };
        let closed = view.downgrade();
        theme::dialog(dialog, cx)
            .title(file.clone())
            .w(px(880.))
            .on_close(move |_, _, cx| {
                let _ = closed.update(cx, |view, cx| {
                    view.shot_open = None;
                    cx.notify();
                });
            })
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        div()
                            .debug_selector(|| "shot-viewer".into())
                            .w_full()
                            .h(px(480.))
                            .rounded(px(10.))
                            .overflow_hidden()
                            .bg(colors.surface_subtle)
                            .child(img(path).size_full().object_fit(ObjectFit::Contain)),
                    )
                    .child(div().text_xs().text_color(colors.muted).child(format!(
                        "{} / {} · {}",
                        at + 1,
                        total,
                        taken(shot)
                    ))),
            )
            .footer(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(
                        h_flex()
                            .flex_1()
                            .gap_2()
                            .child(
                                Key::new("shot-prev")
                                    .icon(Icon::new(UiIcon::Back))
                                    .white()
                                    .debug_selector(|| "shot-prev".into())
                                    .on_click(step(-1)),
                            )
                            .child(
                                Key::new("shot-next")
                                    .icon(Icon::new(UiIcon::Next))
                                    .white()
                                    .debug_selector(|| "shot-next".into())
                                    .on_click(step(1)),
                            ),
                    )
                    // ia[instance.screenshots]: 复制截图 | 大图弹窗「复制图片」 | 图片进剪贴板，toast“已复制图片”
                    .child(
                        Key::new("shot-copy")
                            .label("复制图片")
                            .white()
                            .debug_selector(|| "shot-copy".into())
                            .on_click(on(|view, file, window, cx| {
                                (view.handler)(
                                    InstanceIntent::CopyScreenshot(file.to_owned()),
                                    window,
                                    cx,
                                );
                            })),
                    )
                    // ia[instance.screenshots]: 在访达中显示截图 | 大图弹窗「在访达中显示」 | 打开 screenshots 文件夹并选中该文件
                    .child(
                        Key::new("shot-reveal")
                            .label(crate::platform::reveal_label())
                            .white()
                            .debug_selector(|| "shot-reveal".into())
                            .on_click(on(|view, file, window, cx| {
                                (view.handler)(
                                    InstanceIntent::RevealPath(format!("screenshots/{file}")),
                                    window,
                                    cx,
                                );
                            })),
                    )
                    // ia[instance.screenshots]: 删除截图 | 大图弹窗「删除」→ 警告弹窗 | 删除文件，之后不能找回；游戏运行时也可以删
                    .child(
                        Key::new("shot-delete")
                            .label("删除")
                            .white()
                            .debug_selector(|| "shot-delete".into())
                            .on_click(on(|view, file, window, cx| {
                                view.shot_open = None;
                                window.close_dialog(cx);
                                view.ask_confirm(
                                    Confirm::DeleteScreenshot(file.to_owned()),
                                    window,
                                    cx,
                                );
                            })),
                    ),
            )
    }
}

/// When a picture was taken, in words.
fn taken(shot: &ScreenshotInfo) -> String {
    let seconds = u64::try_from(shot.modified_ms / 1000).unwrap_or(0);
    clock(seconds)
}
