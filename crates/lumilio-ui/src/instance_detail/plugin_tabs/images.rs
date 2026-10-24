use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, ObjectFit, RenderImage, Window, div, img, px};
use gpui_component::{StyledExt as _, h_flex, v_flex};
use lumilio_plugin_api::{ImageData, KeyKind, ListItem, Tone, View};

use super::super::InstanceDetailView;
use super::super::panels::act;
use super::{COVER, THUMB};
use crate::key::Key;
use crate::kit::TagKind;
use crate::theme::ShellColors;
use crate::{kit, theme};

/// Every image in the tree, in the order the renderer meets them.
pub(super) fn collect_images<'a>(view: &'a View, out: &mut Vec<&'a ImageData>) {
    match view {
        View::Section { children, .. } => {
            children.iter().for_each(|child| collect_images(child, out))
        }
        View::List { items } => out.extend(items.iter().filter_map(|item| item.image.as_ref())),
        View::Detail {
            image, children, ..
        } => {
            out.extend(image.iter());
            children.iter().for_each(|child| collect_images(child, out));
        }
        View::Image(image) => out.push(image),
        View::Map
        | View::Studio { .. }
        | View::Model { .. }
        | View::Table { .. }
        | View::Empty { .. }
        | View::Text { .. }
        | View::Tags(_)
        | View::Key { .. } => {}
    }
}

pub(super) fn collect_models<'a>(view: &'a View, out: &mut Vec<&'a str>) {
    match view {
        View::Model { file } => out.push(file),
        View::Section { children, .. } | View::Detail { children, .. } => {
            children.iter().for_each(|child| collect_models(child, out));
        }
        _ => {}
    }
}

/// RGBA pixels from a plugin as the bitmap GPUI uploads; a malformed image is
/// left out rather than trusted.
pub(super) fn render_image(data: &ImageData) -> Option<Arc<RenderImage>> {
    if !data.is_valid() {
        return None;
    }
    crate::pixels::image(data.width, data.height, &data.rgba)
}

pub(super) struct Paint<'a> {
    pub(super) plugin: &'a str,
    pub(super) images: &'a [Option<Arc<RenderImage>>],
    pub(super) models: &'a [Entity<crate::model_view::ModelView>],
    pub(super) maps: &'a [Entity<crate::world_explorer::MapView>],
    pub(super) studios: &'a [Entity<crate::schematic_studio::StudioView>],
    pub(super) next_map: usize,
    pub(super) next_studio: usize,
    pub(super) next_model: usize,
    pub(super) next_table: usize,
    pub(super) next_image: usize,
    pub(super) next_id: usize,
    pub(super) colors: ShellColors,
}

pub(super) fn picture_box(picture: Arc<RenderImage>, size: f32, colors: ShellColors) -> gpui::Div {
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(6.))
        .overflow_hidden()
        .bg(colors.surface_subtle)
        .child(img(picture).size_full().object_fit(ObjectFit::Cover))
}

impl Paint<'_> {
    pub(super) fn id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id
    }

    pub(super) fn image(&mut self) -> Option<Arc<RenderImage>> {
        let image = self.images.get(self.next_image).cloned().flatten();
        self.next_image += 1;
        image
    }

    pub(super) fn view(&mut self, view: &View, cx: &mut Context<InstanceDetailView>) -> AnyElement {
        let colors = self.colors;
        match view {
            View::Section { title, children } => {
                let body = v_flex()
                    .w_full()
                    .gap_3()
                    .children(children.iter().map(|child| self.view(child, cx)));
                kit::section(title.clone(), colors, body).into_any_element()
            }
            View::List { items } => {
                let rows: Vec<AnyElement> = items.iter().map(|item| self.item(item, cx)).collect();
                kit::list(rows, colors).into_any_element()
            }
            View::Detail {
                title,
                subtitle,
                image,
                facts,
                children,
            } => {
                let picture = image.as_ref().and_then(|_| self.image());
                // Consume the original tree order before moving entry keys into
                // the header, so nested images/models keep their asset identity.
                let mut actions = Vec::new();
                let mut body = Vec::new();
                for child in children {
                    let element = self.view(child, cx);
                    if matches!(child, View::Key { .. } | View::Model { .. }) {
                        actions.push(element);
                    } else {
                        body.push(element);
                    }
                }
                let head = h_flex()
                    .w_full()
                    .gap_4()
                    .items_center()
                    .children(
                        picture
                            .map(|picture| picture_box(picture, COVER, colors).into_any_element()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                h_flex()
                                    .w_full()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id(("plugin-detail-title", self.id()))
                                            .debug_selector(|| "plugin-detail-title".into())
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_lg()
                                            .font_semibold()
                                            .text_color(colors.foreground)
                                            .child(title.clone()),
                                    )
                                    .child(h_flex().flex_none().gap_2().children(actions)),
                            )
                            .children(
                                subtitle.clone().map(|text| {
                                    div().text_sm().text_color(colors.muted).child(text)
                                }),
                            ),
                    );
                let rows: Vec<AnyElement> = facts
                    .iter()
                    .map(|(label, value)| {
                        let id = self.id();
                        kit::value_row(
                            ("plugin-fact", id),
                            label.clone(),
                            None,
                            value.clone(),
                            None,
                            colors,
                        )
                        .into_any_element()
                    })
                    .collect();
                let mut column = v_flex().w_full().gap_4().child(head);
                if !rows.is_empty() {
                    column = column.child(kit::list(rows, colors));
                }
                column = column.children(body);
                column.into_any_element()
            }
            View::Table { columns, rows } => {
                let line = |cells: &[String], head: bool| {
                    h_flex()
                        .w_full()
                        .gap_3()
                        .py(px(8.))
                        .children(cells.iter().enumerate().map(|(index, cell)| {
                            let text = div()
                                .flex_1()
                                .min_w_0()
                                .when(index > 0, gpui::Styled::flex_none)
                                .when(index > 0, |cell| cell.w(px(96.)).text_right())
                                .text_xs()
                                .child(cell.clone());
                            if head {
                                text.text_color(colors.muted)
                            } else {
                                text.font_family(theme::mono_font())
                                    .text_color(colors.foreground)
                            }
                            .into_any_element()
                        }))
                };
                let table = self.next_table;
                self.next_table += 1;
                // Header and material rows follow the detail pane's scroll.
                let body = div()
                    .id(("plugin-table", table))
                    .debug_selector(move || format!("plugin-table-{table}"))
                    .w_full()
                    .children(rows.iter().map(|row| {
                        div()
                            .w_full()
                            .border_b_1()
                            .border_color(colors.border)
                            .child(line(row, false))
                    }));
                v_flex()
                    .w_full()
                    .child(kit::list(vec![line(columns, true)], colors))
                    .child(body)
                    .into_any_element()
            }
            View::Empty { title, message } => {
                kit::empty(title.clone(), message.clone(), colors).into_any_element()
            }
            View::Text { text, tone } => {
                let base = div().text_sm().child(text.clone());
                match tone {
                    Tone::Body => base.text_color(colors.foreground),
                    Tone::Secondary => base.text_color(colors.muted),
                    Tone::Mono => base
                        .text_color(colors.foreground)
                        .font_family(theme::mono_font()),
                }
                .into_any_element()
            }
            View::Tags(tags) => h_flex()
                .gap_1()
                .flex_wrap()
                .children(
                    tags.iter()
                        .map(|tag| kit::tag(tag.clone(), TagKind::Plain, colors)),
                )
                .into_any_element(),
            View::Image(_) => match self.image() {
                Some(picture) => picture_box(picture, COVER, colors).into_any_element(),
                None => div().into_any_element(),
            },
            View::Map => {
                let map = self.maps.get(self.next_map);
                self.next_map += 1;
                map.map_or_else(
                    || div().into_any_element(),
                    |map| map.clone().into_any_element(),
                )
            }
            View::Studio { .. } => {
                let studio = self.studios.get(self.next_studio);
                self.next_studio += 1;
                studio.map_or_else(
                    || div().into_any_element(),
                    |studio| studio.clone().into_any_element(),
                )
            }
            View::Model { .. } => {
                let model = self.models.get(self.next_model);
                self.next_model += 1;
                model.map_or_else(
                    || div().into_any_element(),
                    |model| model.clone().into_any_element(),
                )
            }
            View::Key {
                id,
                label,
                kind,
                destructive,
            } => {
                let key_id = self.id();
                let plugin = self.plugin.to_owned();
                let (action, ask) = (id.clone(), destructive.then(|| label.clone()));
                let key = Key::new(("plugin-key", key_id)).label(label.clone());
                let key = match kind {
                    KeyKind::Primary => key.primary(),
                    KeyKind::Ghost => key.ghost(),
                };
                let on_click = act(cx, move |view, window, cx| {
                    view.plugin_act(plugin.clone(), action.clone(), ask.clone(), window, cx);
                });
                let selector = format!("plugin-key-{}", id.0);
                key.on_click(move |_, window: &mut Window, cx: &mut App| on_click(window, cx))
                    .debug_selector(move || selector.clone())
                    .into_any_element()
            }
        }
    }

    pub(super) fn item(
        &mut self,
        item: &ListItem,
        cx: &mut Context<InstanceDetailView>,
    ) -> AnyElement {
        let colors = self.colors;
        let id = self.id();
        let lead = item
            .image
            .as_ref()
            .and_then(|_| self.image())
            .map(|picture| picture_box(picture, THUMB, colors).into_any_element());
        let trail = (!item.tags.is_empty() || item.value.is_some()).then(|| {
            h_flex()
                .gap_2()
                .items_center()
                .children(
                    item.tags
                        .iter()
                        .map(|tag| kit::tag(tag.clone(), TagKind::Plain, colors)),
                )
                .children(item.value.clone().map(|value| {
                    div()
                        .font_family(theme::mono_font())
                        .text_xs()
                        .text_color(colors.muted)
                        .child(value)
                }))
                .into_any_element()
        });
        let row = kit::row(
            item.title.clone(),
            item.subtitle.clone().unwrap_or_default(),
            lead,
            trail,
            colors,
        );
        let selector = format!("plugin-item-{}", item.id);
        let wrapper = div()
            .id(("plugin-item", id))
            .debug_selector(move || selector.clone())
            .child(row);
        match &item.open {
            Some(action) => {
                let (plugin, action) = (self.plugin.to_owned(), action.clone());
                wrapper
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.plugin_act(plugin.clone(), action.clone(), None, window, cx);
                    }))
                    .into_any_element()
            }
            None => wrapper.into_any_element(),
        }
    }
}
