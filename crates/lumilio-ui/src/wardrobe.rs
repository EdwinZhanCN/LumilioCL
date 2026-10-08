//! Retained wardrobe for one account detail. The application owns I/O; this
//! entity owns pending feedback and emits stable target-bound operations.
use crate::{
    key::Key,
    kit,
    live::{LiveHandler, LiveIntent},
    new_game::Failure,
    theme::ShellColors,
    tr, tr_all,
};
use gpui::{Context, IntoElement, Render, Window, div, prelude::*};
use gpui_component::{ActiveTheme as _, Sizable as _, StyledExt as _, h_flex, v_flex};
use lumilio_core::{LibrarySkin, MojangProfile, SkinModel, SkinSource};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WardrobeAction {
    Reload,
    Import(Vec<PathBuf>),
    SaveCurrent,
    Wear(String),
    Default,
    Cape(Option<String>),
    Model(String, SkinModel),
    Reorder(Vec<String>),
    Remove(String),
}

pub struct Wardrobe {
    key: String,
    revision: u64,
    handler: LiveHandler,
    microsoft: bool,
    profile: Option<MojangProfile>,
    library: Vec<LibrarySkin>,
    busy: bool,
    loaded: bool,
    error: Option<Failure>,
    profile_error: Option<Failure>,
    pending: Option<WardrobeAction>,
    before: Option<MojangProfile>,
}

impl Wardrobe {
    #[cfg(test)]
    pub(crate) fn is_loaded(&self) -> bool {
        self.loaded
    }
    pub fn new(key: String, revision: u64, microsoft: bool, handler: LiveHandler) -> Self {
        Self {
            key,
            revision,
            handler,
            microsoft,
            profile: None,
            library: Vec::new(),
            busy: false,
            loaded: false,
            error: None,
            profile_error: None,
            pending: None,
            before: None,
        }
    }

    fn request(&mut self, action: WardrobeAction, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.error = None;
        (self.handler)(
            LiveIntent::Wardrobe {
                key: self.key.clone(),
                revision: self.revision,
                action,
            },
            window,
            cx,
        );
        cx.notify();
    }

    pub fn listed(
        &mut self,
        library: Result<Vec<LibrarySkin>, Failure>,
        profile: Option<Result<MojangProfile, Failure>>,
        cx: &mut Context<Self>,
    ) {
        self.loaded = true;
        match library {
            Ok(entries) => self.library = entries,
            Err(error) => self.error = Some(error),
        }
        if let Some(result) = profile {
            match result {
                Ok(profile) => {
                    self.profile = Some(profile);
                    self.profile_error = None;
                }
                Err(error) => self.profile_error = Some(error),
            }
        }
        cx.notify();
    }

    pub fn finished(
        &mut self,
        action: WardrobeAction,
        result: Result<(), Failure>,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        match result {
            Err(error) => self.error = Some(error),
            Ok(()) => {
                self.error = None;
                if self.microsoft
                    && matches!(
                        action,
                        WardrobeAction::Wear(_) | WardrobeAction::Default | WardrobeAction::Cape(_)
                    )
                {
                    self.before = self.profile.clone();
                    self.pending = Some(action);
                    self.busy = true;
                }
            }
        }
        cx.notify();
    }

    pub fn pending_skin(&self) -> Option<String> {
        match &self.pending {
            Some(WardrobeAction::Wear(id)) => Some(id.clone()),
            _ => None,
        }
    }

    pub fn confirmed(
        &mut self,
        result: Result<MojangProfile, Failure>,
        matches: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        self.busy = false;
        match result {
            Ok(profile) => {
                let confirmed = match &self.pending {
                    Some(WardrobeAction::Default) => profile.active_skin().is_none(),
                    Some(WardrobeAction::Cape(id)) => {
                        profile.active_cape().map(|cape| &cape.id) == id.as_ref()
                    }
                    Some(WardrobeAction::Wear(id)) => matches.unwrap_or_else(|| {
                        self.library
                            .iter()
                            .find(|entry| &entry.id == id)
                            .is_some_and(|entry| {
                                profile
                                    .active_skin()
                                    .is_some_and(|skin| skin.model == entry.model)
                                    && self.before.as_ref().and_then(MojangProfile::active_skin)
                                        != profile.active_skin()
                            })
                    }),
                    _ => true,
                };
                if confirmed {
                    self.pending = None;
                    self.before = None;
                }
                self.profile = Some(profile);
                self.profile_error = None;
            }
            Err(error) => self.error = Some(error),
        }
        cx.notify();
    }

    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: &'static str,
        action: WardrobeAction,
        enabled: bool,
        cx: &Context<Self>,
    ) -> Key {
        let weak = cx.weak_entity();
        Key::new(id)
            .label(label)
            .white()
            .disabled(self.busy || !enabled)
            .on_click(move |_, window, cx| {
                let _ = weak.update(cx, |view, cx| view.request(action.clone(), window, cx));
            })
    }
}

impl Render for Wardrobe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::from_theme(cx.theme());
        let weak = cx.weak_entity();
        let pick = weak.clone();
        // ia[accounts]: 导入皮肤 | 衣橱「导入 PNG…」或拖入 PNG | 验证并复制到本地库，旧版贴图升级；选择手臂模型后穿戴
        let import = Key::new("wardrobe-import")
            .label(tr!("wardrobe-import"))
            .white()
            .disabled(self.busy)
            .debug_selector(|| "wardrobe-import".into())
            .on_click(move |_, window, cx| {
                let chosen =
                    crate::platform::pick_path(cx, true, false, tr!("account-skin-pick-picture"));
                let handle = window.window_handle();
                let weak = pick.clone();
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
            });
        let mut content = v_flex()
            .id("wardrobe")
            .debug_selector(|| "wardrobe".into())
            .w_full()
            .min_w_0()
            .gap_4()
            .on_drop::<gpui::ExternalPaths>(move |paths, window, cx| {
                let _ = weak.update(cx, |view, cx| {
                    view.request(WardrobeAction::Import(paths.paths().to_vec()), window, cx)
                });
            });
        if self.microsoft {
            // ia[accounts]: 更换正版外观 | 衣橱当前皮肤、恢复默认、披风列表 | 用该账户的 Mojang 档案上传与切换；接受后延迟确认，旧档案显示同步中
            let profile_ready = self.profile.is_some() && self.profile_error.is_none();
            content = content.child(
                v_flex()
                    .gap_2()
                    .child(div().text_sm().font_medium().child(tr!("wardrobe-current")))
                    .child(
                        div().text_xs().text_color(colors.muted).child(
                            self.profile
                                .as_ref()
                                .and_then(MojangProfile::active_skin)
                                .map_or(tr!("account-skin-kind-default"), |skin| {
                                    if skin.model == SkinModel::Slim {
                                        tr!("account-skin-model-slim")
                                    } else {
                                        tr!("account-skin-model-classic")
                                    }
                                }),
                        ),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                self.button(
                                    "wardrobe-save-current",
                                    tr!("wardrobe-save-current"),
                                    WardrobeAction::SaveCurrent,
                                    profile_ready
                                        && self
                                            .profile
                                            .as_ref()
                                            .and_then(MojangProfile::active_skin)
                                            .is_some(),
                                    cx,
                                ),
                            )
                            .child(self.button(
                                "wardrobe-default",
                                tr!("wardrobe-default"),
                                WardrobeAction::Default,
                                profile_ready,
                                cx,
                            )),
                    ),
            );
            let mut capes = v_flex()
                .gap_2()
                .child(div().text_sm().font_medium().child(tr!("wardrobe-capes")))
                .child(self.button(
                    "wardrobe-no-cape",
                    tr!("wardrobe-no-cape"),
                    WardrobeAction::Cape(None),
                    profile_ready,
                    cx,
                ));
            if let Some(profile) = &self.profile {
                for cape in &profile.capes {
                    let chosen = cape.state == "ACTIVE";
                    let id = cape.id.clone();
                    let weak = cx.weak_entity();
                    capes = capes.child(
                        Key::new(gpui::ElementId::Name(format!("wardrobe-cape-{id}").into()))
                            .label(cape.alias.clone())
                            .white()
                            .disabled(self.busy || !profile_ready || chosen)
                            .on_click(move |_, window, cx| {
                                let _ = weak.update(cx, |view, cx| {
                                    view.request(WardrobeAction::Cape(Some(id.clone())), window, cx)
                                });
                            }),
                    );
                }
            }
            content = content.child(capes);
        }
        let mut library = v_flex().gap_3().child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .text_sm()
                        .font_medium()
                        .child(tr!("wardrobe-library")),
                )
                .child(import)
                .child(
                    self.button(
                        "wardrobe-reload",
                        tr!("wardrobe-reload"),
                        WardrobeAction::Reload,
                        true,
                        cx,
                    )
                    .debug_selector(|| "wardrobe-reload".into()),
                ),
        );
        if !self.loaded || self.library.is_empty() {
            library = library.child(div().text_xs().text_color(colors.muted).child(
                if self.loaded {
                    tr!("wardrobe-empty")
                } else {
                    tr!("wardrobe-loading")
                },
            ));
        }
        // ia[accounts]: 管理本地库 | 每张皮肤的模型、穿戴与 ⋯ 菜单 | 模型、来源和顺序持久保存；移除库条目不破坏离线账户已选的文件
        for (ix, skin) in self.library.iter().enumerate() {
            let weak = cx.weak_entity();
            let id = skin.id.clone();
            let source = match &skin.source {
                SkinSource::LocalFile(path) => path.display().to_string(),
                SkinSource::Mojang(url) => url.clone(),
            };
            let mut menu = Vec::new();
            for (label, to) in [
                (tr!("wardrobe-up"), ix.checked_sub(1)),
                (
                    tr!("wardrobe-down"),
                    (ix + 1 < self.library.len()).then_some(ix + 1),
                ),
            ] {
                if let Some(to) = to {
                    let mut ids: Vec<_> = self.library.iter().map(|skin| skin.id.clone()).collect();
                    ids.swap(ix, to);
                    let weak = cx.weak_entity();
                    menu.push(kit::MenuEntry::new(label, move |window, cx| {
                        let _ = weak.update(cx, |view, cx| {
                            view.request(WardrobeAction::Reorder(ids.clone()), window, cx)
                        });
                    }));
                }
            }
            let remove = skin.id.clone();
            let remove_view = cx.weak_entity();
            menu.push(
                kit::MenuEntry::new(tr!("common-remove"), move |window, cx| {
                    let _ = remove_view.update(cx, |view, cx| {
                        view.request(WardrobeAction::Remove(remove.clone()), window, cx)
                    });
                })
                .danger(),
            );
            library = library.child(
                v_flex()
                    .id(gpui::ElementId::Name(
                        format!("wardrobe-entry-{}", skin.id).into(),
                    ))
                    .gap_2()
                    .min_w_0()
                    .child(div().text_sm().font_medium().child(skin.name.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(colors.muted)
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(source),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(kit::segments(
                                "wardrobe-model",
                                tr_all!["account-skin-model-classic", "account-skin-model-slim"],
                                usize::from(skin.model == SkinModel::Slim),
                                move |ix, window, cx| {
                                    let _ = weak.update(cx, |view, cx| {
                                        view.request(
                                            WardrobeAction::Model(
                                                id.clone(),
                                                if ix == 1 {
                                                    SkinModel::Slim
                                                } else {
                                                    SkinModel::Wide
                                                },
                                            ),
                                            window,
                                            cx,
                                        )
                                    });
                                },
                            ))
                            .child(
                                self.button(
                                    "wardrobe-wear",
                                    tr!("wardrobe-wear"),
                                    WardrobeAction::Wear(skin.id.clone()),
                                    !self.microsoft
                                        || self.profile_error.is_none() && self.profile.is_some(),
                                    cx,
                                )
                                .debug_selector({
                                    let id = skin.id.clone();
                                    move || format!("wardrobe-wear-{id}")
                                }),
                            )
                            .child(kit::more_menu("wardrobe-more", menu, colors)),
                    ),
            );
        }
        content
            .child(library)
            .children(self.pending.as_ref().map(|_| {
                div()
                    .text_xs()
                    .text_color(colors.muted)
                    .child(tr!("wardrobe-pending"))
            }))
            .children(
                self.profile_error
                    .clone()
                    .into_iter()
                    .chain(self.error.clone())
                    .map(|(message, detail)| {
                        v_flex()
                            .gap_2()
                            .child(div().text_sm().text_color(colors.danger).child(message))
                            .child(kit::technical("wardrobe-technical", detail).xsmall())
                    }),
            )
    }
}

#[cfg(test)]
mod tests;
