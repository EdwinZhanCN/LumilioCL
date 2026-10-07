use super::{Load, Viewer};
use crate::{key::Key, theme};
use gpui::{
    App, Context, Entity, FocusHandle, Subscription, WeakEntity, Window, div, prelude::*, px,
};
use gpui_component::{WindowExt as _, dialog::Dialog};

/// The page retains only an entry point. Rendering and asset loading exist
/// solely for the current modal session; closing drops its worker and cursor guard.
pub struct ModelView {
    load: Load,
    viewer: Option<WeakEntity<Viewer>>,
    trigger: FocusHandle,
    modal_focus: Option<Subscription>,
}

impl ModelView {
    pub(crate) fn new(load: Load, cx: &mut Context<Self>) -> Self {
        Self {
            load,
            viewer: None,
            trigger: cx.focus_handle(),
            modal_focus: None,
        }
    }

    pub(crate) fn request_id(&self) -> Option<u64> {
        self.viewer().map(|viewer| viewer.entity_id().as_u64())
    }

    fn viewer(&self) -> Option<Entity<Viewer>> {
        self.viewer.as_ref()?.upgrade()
    }

    pub(crate) fn assets(
        &mut self,
        result: Result<lumilio_core::ModelPreview, String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(viewer) = self.viewer() {
            viewer.update(cx, |viewer, cx| viewer.assets(result, cx));
        }
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.viewer().is_some() && window.has_active_dialog(cx) {
            return;
        }
        self.viewer = None;
        window.focus(&self.trigger, cx);
        let viewer = cx.new(|cx| Viewer::new(self.load.clone(), cx));
        // The dialog owns the session; programmatic close can skip focus events.
        self.viewer = Some(viewer.downgrade());
        let owner = cx.weak_entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            let Some(owner) = owner.upgrade() else {
                window.defer(cx, |window, cx| window.close_dialog(cx));
                return dialog;
            };
            Self::dialog(&owner, viewer.clone(), dialog, window, cx)
        });
        if let Some(focus) = window.focused(cx) {
            self.modal_focus = Some(cx.on_focus_out(&focus, window, |owner, _, window, cx| {
                // Focus changes stop captured movement. Dialog ownership also
                // releases the session when close skips these events.
                if let Some(viewer) = owner.viewer() {
                    viewer.update(cx, |viewer, _| viewer.release_capture());
                }
                if !window.has_active_dialog(cx) {
                    owner.viewer = None;
                    owner.modal_focus = None;
                }
                cx.notify();
            }));
        }
        cx.notify();
    }

    fn dialog(
        owner: &Entity<Self>,
        viewer: Entity<Viewer>,
        dialog: Dialog,
        window: &mut Window,
        cx: &mut App,
    ) -> Dialog {
        let closed = owner.downgrade();
        let cancelled = owner.downgrade();
        theme::dialog(dialog, cx)
            .title("3D 投影预览")
            .w(px(
                (f32::from(window.viewport_size().width) - 48.).clamp(280., 1120.)
            ))
            .overlay_closable(false)
            // A viewer is not a form: an unhandled Enter must never dismiss it.
            .on_ok(|_, _, _| false)
            // ia[instance]: 退出 3D 投影预览 | 弹窗 Esc / 关闭按钮 | 捕获鼠标时 Esc 先释放并停止移动，再按 Esc 关闭弹窗；关闭后焦点回到入口并释放渲染资源
            .on_cancel(move |_, _, cx| {
                cancelled
                    .update(cx, |owner, cx| {
                        let captured = owner
                            .viewer()
                            .is_some_and(|viewer| viewer.read(cx).capture.is_some());
                        if captured {
                            if let Some(viewer) = owner.viewer() {
                                viewer.update(cx, |viewer, cx| {
                                    viewer.release_capture();
                                    cx.notify();
                                });
                            }
                            false
                        } else {
                            owner.viewer = None;
                            owner.modal_focus = None;
                            cx.notify();
                            true
                        }
                    })
                    .unwrap_or(true)
            })
            .on_close(move |_, _, cx| {
                let _ = closed.update(cx, |owner, cx| {
                    if let Some(viewer) = owner.viewer.take().and_then(|viewer| viewer.upgrade()) {
                        viewer.update(cx, |viewer, _| viewer.release_capture());
                    }
                    owner.modal_focus = None;
                    cx.notify();
                });
            })
            .child(viewer)
    }
}

impl Render for ModelView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ia[instance]: 打开 3D 投影预览 | 投影详情标题同行的顶部按钮组「打开 3D 预览」 | 打开观察弹窗后才读取投影和游戏贴图；加载、错误与重试均在弹窗内；游戏版本画不出的方块在技术详情中列出
        let open = Key::new(("model-open", cx.entity_id().as_u64()))
            .label("打开 3D 预览")
            .white()
            .debug_selector(|| "model-open".into())
            .on_click(cx.listener(|this, _, window, cx| this.open(window, cx)));
        div()
            .track_focus(&self.trigger)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.open(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(open)
    }
}
