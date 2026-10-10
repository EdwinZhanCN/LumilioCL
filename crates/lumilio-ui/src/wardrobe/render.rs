//! The look area of an account's detail, in two layers (after Modrinth's
//! `Skins.vue`): the figure with one place to act under it, beside a grid of
//! skin pictures. Editing replaces the grid; nothing else is on the page.
use super::*;
use crate::{assets::UiIcon, theme};
use gpui::{AnyElement, ElementId};
use gpui_component::{Icon, IconName};

/// The narrowest window in which the preview stays put beside a grid that
/// scrolls on its own: the account list, the preview and three pictures,
/// within the content column's padding. Narrower, everything stacks.
pub(crate) const PINNED_PREVIEW_WIDTH: f32 = 968.;
/// The preview column's width.
pub(crate) const PREVIEW_WIDTH: f32 = 280.;

/// A picture well: the drawn picture, or an empty well while it is drawn.
/// The chosen one is ringed. It takes focus, and Enter or Space clicks it
/// (GPUI's keyboard click on a focused element).
pub(super) fn tile(
    id: impl Into<ElementId>,
    (w, h): (f32, f32),
    image: Option<&Arc<RenderImage>>,
    chosen: bool,
    colors: ShellColors,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .relative()
        .flex_none()
        .w(px(w))
        .h(px(h))
        .rounded_md()
        .overflow_hidden()
        .bg(colors.surface_subtle)
        .border_2()
        .border_color(if chosen {
            colors.primary
        } else {
            gpui::transparent_black()
        })
        .when(!chosen, |tile| {
            tile.hover(|tile| tile.border_color(colors.border))
        })
        .focusable()
        .tab_stop(true)
        .focus_visible(move |tile| tile.border_color(colors.focus))
        .cursor_pointer()
        .children(image.map(|image| {
            img(image.clone())
                .size_full()
                .object_fit(ObjectFit::Contain)
        }))
}

/// The tick on the skin the account wears.
fn worn_badge(colors: ShellColors) -> impl IntoElement {
    div()
        .absolute()
        .top_1()
        .right_1()
        .size(px(20.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(colors.primary)
        .child(
            Icon::new(IconName::Check)
                .size(px(13.))
                .text_color(colors.primary_foreground),
        )
}

/// Opens the file picker and imports the chosen PNG into the library.
fn pick_import(weak: WeakEntity<Wardrobe>, window: &mut Window, cx: &mut App) {
    if weak.read_with(cx, |view, _| view.busy).unwrap_or(true) {
        return;
    }
    let chosen = crate::platform::pick_path(cx, true, false, tr!("account-skin-pick-picture"));
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        if let Some(path) = chosen.await {
            let _ = cx.update_window(handle, |_, window, cx| {
                let _ = weak.update(cx, |view, cx| {
                    view.request(WardrobeAction::Import(vec![path]), window, cx)
                });
            });
        }
    })
    .detach();
}

impl Wardrobe {
    /// What can be done under the figure: edit it, or settle a trial.
    fn preview_actions(&self, colors: ShellColors, cx: &Context<Self>) -> Option<AnyElement> {
        if self.editor.is_some() || self.offline_editing {
            return None;
        }
        let row = h_flex().w_full().justify_center().gap_2();
        if let Some(entry) = self
            .trying
            .as_ref()
            .and_then(|id| self.library.iter().find(|entry| &entry.id == id))
        {
            let id = entry.id.clone();
            let cancel = cx.weak_entity();
            return Some(
                row
                    // ia[accounts]: 还原试穿 | 人物下的「还原」 | 预览回到账户自己的外观，账户与库都不变
                    .child(
                        Key::new("wardrobe-trial-cancel")
                            .label(tr!("wardrobe-trial-reset"))
                            .white()
                            .debug_selector(|| "wardrobe-trial-cancel".into())
                            .on_click(move |_, _, cx| {
                                let _ = cancel.update(cx, |view, cx| view.try_on(None, cx));
                            }),
                    )
                    // ia[accounts]: 应用试穿的皮肤 | 人物下的「应用」 | 正版账户提交这张皮肤及其搭配披风，接受后显示同步中；离线账户保存为当前选择
                    .child(
                        self.button(
                            "wardrobe-wear",
                            tr!("wardrobe-trial-wear"),
                            WardrobeAction::Wear(id.clone()),
                            self.can_wear(),
                            cx,
                        )
                        .primary()
                        .debug_selector(move || format!("wardrobe-wear-{id}")),
                    )
                    .into_any_element(),
            );
        }
        let edit_view = cx.weak_entity();
        // ia[accounts]: 编辑外观 | 人物下的「编辑外观…」 | 右侧换成编辑面：纹理、手臂与披风成草稿，应用前不更改账户或库；离线账户在这里选皮肤来源
        let edit = Key::new("wardrobe-edit")
            .label(tr!("wardrobe-edit"))
            .white()
            .debug_selector(|| "wardrobe-edit".into())
            .focus_handle(self.edit_focus.clone())
            .disabled(self.busy || self.microsoft && self.profile.is_none())
            .on_click(move |_, window, cx| {
                let _ = edit_view.update(cx, |view, cx| view.open_editor(window, cx));
            });
        Some(
            row.child(edit)
                .children(self.microsoft.then(|| self.look_menu(colors, cx)))
                .into_any_element(),
        )
    }

    /// The account's own look, beyond editing it.
    fn look_menu(&self, colors: ShellColors, cx: &Context<Self>) -> impl IntoElement {
        let profile_ready = self.profile.is_some()
            && self.profile_error.is_none()
            && self.profile_warning.is_none();
        let save_view = cx.weak_entity();
        let default_view = cx.weak_entity();
        let refresh_view = cx.weak_entity();
        // ia[accounts]: 保存当前皮肤 | 人物下的 ⋯ 菜单「保存当前皮肤到皮肤库」 | 将当前贴图复制到皮肤库，不改变账户
        let save = kit::MenuEntry::new(tr!("wardrobe-save-current"), move |window, cx| {
            let _ = save_view.update(cx, |view, cx| {
                view.request(WardrobeAction::SaveCurrent, window, cx)
            });
        })
        .disabled(
            self.busy
                || !profile_ready
                || self
                    .profile
                    .as_ref()
                    .and_then(MojangProfile::active_skin)
                    .is_none(),
        );
        // ia[accounts]: 恢复默认 | 人物下的 ⋯ 菜单「恢复默认皮肤」 | 正版账户恢复默认皮肤，已接受后延迟确认
        let reset = kit::MenuEntry::new(tr!("wardrobe-default"), move |window, cx| {
            let _ = default_view.update(cx, |view, cx| {
                view.request(WardrobeAction::Default, window, cx)
            });
        })
        .disabled(self.busy || !profile_ready);
        // ia[accounts]: 刷新外观 | 人物下的 ⋯ 菜单「刷新外观」 | 强制重新读取档案；冷却未结束时不再请求，失败保留已显示的外观并说明未更新
        let refresh = kit::MenuEntry::new(tr!("wardrobe-reload"), move |window, cx| {
            let _ = refresh_view.update(cx, |view, cx| {
                view.request(WardrobeAction::Reload, window, cx)
            });
        })
        .disabled(self.busy);
        kit::more_menu("wardrobe-look-more", vec![save, reset, refresh], colors)
    }

    /// Syncing, stale and failure notes about the account's look.
    fn status(&self, colors: ShellColors, cx: &Context<Self>) -> impl IntoElement {
        let editing = self.editor.is_some();
        v_flex()
            .w_full()
            .gap_2()
            .children(self.pending.as_ref().map(|_| {
                div()
                    .text_xs()
                    .text_color(colors.muted)
                    .child(tr!("wardrobe-pending"))
            }))
            .children(self.partial.clone().filter(|_| !editing).map(|action| {
                self.button(
                    "wardrobe-retry-cape",
                    tr!("wardrobe-retry-cape"),
                    action,
                    true,
                    cx,
                )
            }))
            .children(self.profile_warning.clone().map(|(_, detail)| {
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .text_color(colors.danger)
                            .child(tr!("wardrobe-stale")),
                    )
                    .child(kit::technical("wardrobe-stale-technical", detail).xsmall())
            }))
            .children(
                (self.profile_warning.is_some() || self.profile_error.is_some()).then(|| {
                    // ia[accounts]: 重试读取外观 | 外观未更新或读取失败旁的「刷新外观」 | 再次强制读取；仍失败时保留上次外观
                    self.button(
                        "wardrobe-reload",
                        tr!("wardrobe-reload"),
                        WardrobeAction::Reload,
                        true,
                        cx,
                    )
                    .debug_selector(|| "wardrobe-reload".into())
                }),
            )
            .children(
                self.profile_error
                    .clone()
                    .into_iter()
                    // The edit panel shows its own failure beside its fields.
                    .chain(self.error.clone().filter(|_| !editing))
                    .map(|(message, detail)| {
                        v_flex()
                            .gap_2()
                            .child(div().text_sm().text_color(colors.danger).child(message))
                            .child(kit::technical("wardrobe-technical", detail).xsmall())
                    }),
            )
    }

    /// The first cell of the grid: choose a PNG, or drop one anywhere here.
    fn add_tile(&self, colors: ShellColors, cx: &Context<Self>) -> impl IntoElement {
        let (w, h) = SKIN_THUMB;
        let click = cx.weak_entity();
        // ia[accounts]: 添加皮肤 | 皮肤网格第一格「添加皮肤」，或把 PNG 拖进外观区 | 验证并复制到皮肤库，旧版贴图升级；之后点它试穿
        v_flex()
            .id("wardrobe-import")
            .debug_selector(|| "wardrobe-import".into())
            .role(gpui::Role::Button)
            .aria_label(tr!("wardrobe-add"))
            .focusable()
            .tab_stop(true)
            .focus_visible(move |tile| tile.border_color(colors.focus))
            .flex_none()
            .w(px(w))
            .h(px(h))
            .items_center()
            .justify_center()
            .gap_1()
            .rounded_md()
            .border_2()
            .border_dashed()
            .border_color(colors.border)
            .cursor_pointer()
            .hover(|tile| tile.bg(colors.surface_subtle))
            .child(
                Icon::new(UiIcon::Plus)
                    .size(px(20.))
                    .text_color(colors.muted),
            )
            .child(
                div()
                    .text_sm()
                    .font_medium()
                    .text_color(colors.foreground)
                    .child(tr!("wardrobe-add")),
            )
            .child(
                div()
                    .px_2()
                    .text_center()
                    .text_xs()
                    .text_color(colors.muted)
                    .child(tr!("wardrobe-add-drop")),
            )
            .on_click(move |_, window, cx| pick_import(click.clone(), window, cx))
    }

    /// One library skin: its picture (click to try it on), its name and a ⋯.
    fn skin_tile(
        &self,
        ix: usize,
        skin: &LibrarySkin,
        colors: ShellColors,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let trying = self.trying.as_ref() == Some(&skin.id);
        let worn = self.worn.contains(&skin.id);
        let toggle = |view: &mut Self, id: String, cx: &mut Context<Self>| {
            let next = (view.trying.as_ref() != Some(&id)).then_some(id);
            view.try_on(next, cx);
        };
        let try_view = cx.weak_entity();
        let try_id = skin.id.clone();
        let id = skin.id.clone();
        // ia[accounts]: 试穿 | 点皮肤网格里的图片 | 人物临时换上这张皮肤（和它搭配的披风），标「试穿中」，下面换成「还原 / 应用」；再点一次还原
        // ia[accounts]: 管理皮肤库 | 每张图片下的 ⋯ 菜单 | 改名、编辑、上下移与移除；顺序持久保存，移除库条目不破坏离线账户已选的文件
        v_flex()
            .id(ElementId::Name(
                format!("wardrobe-entry-{}", skin.id).into(),
            ))
            .w(px(SKIN_THUMB.0))
            .gap_1()
            .child(
                tile(
                    ElementId::Name(format!("wardrobe-try-{}", skin.id).into()),
                    SKIN_THUMB,
                    self.skin_pictures.get(&(skin.id.clone(), skin.model)),
                    trying,
                    colors,
                )
                .role(gpui::Role::Button)
                .aria_label(skin.name.clone())
                .debug_selector(move || format!("wardrobe-try-{id}"))
                .children(worn.then(|| worn_badge(colors)))
                .on_click(move |_, _, cx| {
                    let id = try_id.clone();
                    let _ = try_view.update(cx, |view, cx| toggle(view, id, cx));
                }),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(colors.foreground)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(skin.name.clone()),
                    )
                    .child(kit::small_more_menu(
                        "wardrobe-more",
                        self.menu(ix, skin, cx),
                        colors,
                    )),
            )
    }

    fn library_grid(&self, colors: ShellColors, cx: &Context<Self>) -> AnyElement {
        let mut grid = h_flex()
            .w_full()
            .flex_wrap()
            .items_start()
            .gap_3()
            .child(self.add_tile(colors, cx));
        for (ix, skin) in self.library.iter().enumerate() {
            grid = grid.child(self.skin_tile(ix, skin, colors, cx));
        }
        v_flex()
            .w_full()
            .gap_3()
            .child(kit::section_label(tr!("wardrobe-library"), colors))
            .child(grid)
            .children((!self.loaded).then(|| {
                div()
                    .text_xs()
                    .text_color(colors.muted)
                    .child(tr!("wardrobe-loading"))
            }))
            .into_any_element()
    }

    /// The offline account's skin source, in place of the grid.
    fn offline_form(&self, colors: ShellColors, cx: &Context<Self>) -> AnyElement {
        let cancel = cx.weak_entity();
        let escape = cx.weak_entity();
        // ia[accounts]: 保存离线外观 | 「编辑外观…」后的皮肤来源表单 | 默认、本地皮肤与披风、LittleSkin 或自定义站保存为 SkinChoice；预览更新，启动仍按 ADR 0024
        v_flex()
            .id("wardrobe-offline")
            .track_focus(&self.editor_focus)
            .w_full()
            .gap_4()
            .on_key_down(move |event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    let _ = escape.update(cx, |view, cx| view.cancel_editor(window, cx));
                    cx.stop_propagation();
                }
            })
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_medium()
                            .text_color(colors.foreground)
                            .child(tr!("wardrobe-edit-title")),
                    )
                    .child(
                        kit::ghost("wardrobe-offline-cancel", tr!("common-cancel"), {
                            move |window, cx| {
                                let _ =
                                    cancel.update(cx, |view, cx| view.cancel_editor(window, cx));
                            }
                        })
                        .debug_selector(|| "wardrobe-offline-cancel".into()),
                    ),
            )
            .children(self.offline.clone())
            .into_any_element()
    }
}

impl Render for Wardrobe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        for old in self.old_pictures.drain(..) {
            window.drop_image(old).ok();
        }
        if !self.release_registered {
            cx.on_release_in(window, |this, window, _| {
                for image in this
                    .old_pictures
                    .drain(..)
                    .chain(this.skin_pictures.drain().map(|(_, image)| image))
                    .chain(this.cape_pictures.drain().map(|(_, image)| image))
                {
                    window.drop_image(image).ok();
                }
            })
            .detach();
            self.release_registered = true;
        }
        let colors = ShellColors::current(cx);
        let drop = cx.weak_entity();
        let preview = v_flex()
            .w_full()
            .gap_3()
            .children(self.viewer.upgrade())
            .children(self.preview_actions(colors, cx))
            .child(self.status(colors, cx));
        let side = if self.offline_editing {
            self.offline_form(colors, cx)
        } else if self.editor.is_some() {
            self.render_editor(colors, cx)
        } else {
            self.library_grid(colors, cx)
        };
        let root = div()
            .id("wardrobe")
            .debug_selector(|| "wardrobe".into())
            .on_drop::<gpui::ExternalPaths>(move |paths, window, cx| {
                let _ = drop.update(cx, |view, cx| {
                    view.request(WardrobeAction::Import(paths.paths().to_vec()), window, cx)
                });
            });
        if f32::from(window.viewport_size().width) < PINNED_PREVIEW_WIDTH {
            // Narrow: the page scrolls the stack as a whole.
            return root
                .w_full()
                .child(
                    v_flex()
                        .w_full()
                        .gap_6()
                        .child(div().w(px(PREVIEW_WIDTH)).max_w_full().child(preview))
                        .child(side),
                )
                .into_any_element();
        }
        // With room, the figure stays put and only the grid scrolls, so a skin
        // tried on far down the library is seen at once. In a short window the
        // figure's column scrolls too, rather than running under the
        // navigation.
        root.size_full()
            .min_h_0()
            .child(
                h_flex()
                    .size_full()
                    .min_h_0()
                    .items_start()
                    .gap_8()
                    .child(
                        div()
                            .id("account-preview-scroll")
                            .flex_none()
                            .w(px(PREVIEW_WIDTH))
                            .h_full()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(div().pb(theme::BOTTOM_SAFE_AREA).child(preview)),
                    )
                    .child(
                        div()
                            .id("account-wardrobe-scroll")
                            .debug_selector(|| "account-wardrobe-scroll".into())
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(div().w_full().pb(theme::BOTTOM_SAFE_AREA).child(side)),
                    ),
            )
            .into_any_element()
    }
}
