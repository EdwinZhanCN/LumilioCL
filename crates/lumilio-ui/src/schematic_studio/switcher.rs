//! The workbench's file switcher (plan schematic-studio, T30; D27): a key
//! in the head that names the open file and opens a searchable list of the
//! folder's files. The window has no tree column: browsing the folder is
//! the game page's catalogue, and the picture keeps the width.

use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{Anchor, AnyElement, Context, ObjectFit, RenderImage, SharedString, div, img, px};
use gpui_component::input::Input;
use gpui_component::popover::Popover;
use gpui_component::{Icon, Sizable as _, StyledExt as _, h_flex, v_flex};

use super::{StudioView, tree};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme::ShellColors;
use crate::{theme, tr};

/// One file in the list.
struct Choice {
    path: String,
    name: String,
    /// The folder below the studio folder, empty at its root.
    folder: String,
    chosen: bool,
    /// `≡n` and how many other files share its blocks.
    duplicate: Option<(usize, usize)>,
    thumbnail: Option<Arc<RenderImage>>,
}

/// A schematic's thumbnail (T28), on a quiet square until it is drawn.
fn thumb(picture: Option<Arc<RenderImage>>, side: f32, colors: ShellColors) -> gpui::Div {
    div()
        .flex_none()
        .size(px(side))
        .rounded(px(3.))
        .overflow_hidden()
        .bg(colors.body.key_grey)
        .children(picture.map(|picture| img(picture).size_full().object_fit(ObjectFit::Contain)))
}

impl StudioView {
    pub(super) fn switcher(&self, colors: ShellColors, cx: &mut Context<Self>) -> AnyElement {
        let query = self
            .search
            .as_ref()
            .map(|search| search.read(cx).value().trim().to_lowercase())
            .unwrap_or_default();
        let groups = self.tree.duplicates();
        let choices: Vec<Choice> = self
            .tree
            .files()
            .iter()
            .filter(|path| query.is_empty() || path.to_lowercase().contains(&query))
            .map(|path| {
                let inner = path
                    .strip_prefix(&self.folder)
                    .and_then(|rest| rest.strip_prefix('/'))
                    .unwrap_or(path);
                let folder = inner.rsplit_once('/').map_or("", |(folder, _)| folder);
                Choice {
                    path: path.clone(),
                    name: tree::stem(path).to_owned(),
                    folder: folder.to_owned(),
                    chosen: self.selected.as_deref() == Some(path.as_str()),
                    duplicate: groups
                        .iter()
                        .position(|group| group.contains(path))
                        .map(|index| (index + 1, groups[index].len() - 1)),
                    thumbnail: self.thumbnails.get(path).cloned(),
                }
            })
            .collect();
        let choices = Rc::new(choices);
        let name = self.selected.as_deref().map_or_else(
            || tr!("studio-switch-pick").to_owned(),
            |file| tree::stem(file).to_owned(),
        );
        let trigger = Key::new("studio-switcher")
            .white()
            .large()
            .debug_selector(|| "studio-switcher".into())
            .tooltip(tr!("studio-switch-tip"))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(thumb(
                        self.selected
                            .as_ref()
                            .and_then(|file| self.thumbnails.get(file).cloned()),
                        28.,
                        colors,
                    ))
                    .child(
                        div()
                            .debug_selector(|| "studio-file-name".into())
                            .max_w(px(360.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .text_base()
                            .font_medium()
                            .child(name),
                    )
                    .child(
                        Icon::new(UiIcon::Expand)
                            .size(px(14.))
                            .text_color(colors.muted),
                    ),
            );
        let (studio, search) = (cx.weak_entity(), self.search.clone());
        // ia[plugin.litematica]: 换一份投影 | 工作室头部的文件键（缩略图、文件名、箭头） → 弹出带搜索的文件列表（文件夹、≡ 重复编号） | 换成它的 3D 视口与材料（旧视口的渲染线程与帧随之释放）；读不了的文件就地说明 | 窗口里没有目录树，浏览整个文件夹在游戏页的图鉴里（D27）
        Popover::new("studio-switcher-popover")
            .anchor(Anchor::TopLeft)
            .trigger(trigger)
            .content(move |_, _, cx| {
                let popover = cx.entity();
                let rows = choices.iter().map(|choice| {
                    let (studio, popover, path) =
                        (studio.clone(), popover.clone(), choice.path.clone());
                    let selector = format!("studio-file-{}", choice.path);
                    h_flex()
                        .id(SharedString::from(selector.clone()))
                        .debug_selector(move || selector.clone())
                        .w_full()
                        .gap_2()
                        .items_center()
                        .px_2()
                        .py(px(6.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .when(choice.chosen, |row| row.bg(colors.body.key_grey))
                        .hover(move |row| row.bg(colors.body.key_grey))
                        .on_click(move |_, window, cx| {
                            let _ = studio.update(cx, |view, cx| {
                                view.select(path.clone(), window, cx);
                            });
                            popover.update(cx, |state, cx| state.dismiss(window, cx));
                        })
                        .child(thumb(choice.thumbnail.clone(), 32., colors))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    div()
                                        .truncate()
                                        .text_sm()
                                        .when(choice.chosen, |name| name.font_semibold())
                                        .text_color(colors.foreground)
                                        .child(choice.name.clone()),
                                )
                                .children((!choice.folder.is_empty()).then(|| {
                                    div()
                                        .truncate()
                                        .text_xs()
                                        .text_color(colors.muted)
                                        .child(choice.folder.clone())
                                })),
                        )
                        .children(choice.duplicate.map(|(group, others)| {
                            div()
                                .id(SharedString::from(format!(
                                    "studio-duplicate-{}",
                                    choice.path
                                )))
                                .flex_none()
                                .px(px(4.))
                                .rounded(px(2.))
                                .bg(colors.body.key_grey)
                                .font_family(theme::mono_font())
                                .text_xs()
                                .text_color(colors.foreground)
                                .tooltip(move |window, cx| {
                                    gpui_component::tooltip::Tooltip::new(tr!(
                                        "studio-duplicate-tip",
                                        count = others
                                    ))
                                    .build(window, cx)
                                })
                                .child(format!("≡{group}"))
                        }))
                });
                v_flex()
                    .w(px(340.))
                    .gap_2()
                    .children(search.as_ref().map(|search| {
                        Input::new(search).small().cleanable(true).prefix(
                            Icon::new(UiIcon::Search)
                                .size(px(14.))
                                .text_color(colors.muted),
                        )
                    }))
                    .child(
                        div()
                            .id("studio-switcher-list")
                            .max_h(px(360.))
                            .overflow_y_scroll()
                            .child(v_flex().w_full().gap(px(2.)).children(rows)),
                    )
            })
            .into_any_element()
    }
}
