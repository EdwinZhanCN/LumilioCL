use super::super::panels::{CONTENT_KINDS, CONTENT_LABELS};
use super::super::{InstanceDetailView, InstanceIntent, Section};
use super::model::{BAR_HEIGHT, FILTERS, filter_labels, visible};
use crate::assets::UiIcon;
use crate::key::Key;
use crate::theme::ShellColors;
use crate::{kit, theme, tr, tr_all};
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, Window, div, px};
use gpui_component::IndexPath;
use gpui_component::Sizable as _;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::{Icon, h_flex, v_flex};

fn browse_labels() -> &'static [&'static str] {
    tr_all![
        "instance-content-browse-mod",
        "instance-content-browse-resource-pack",
        "instance-content-browse-shader",
    ]
}

impl InstanceDetailView {
    // ia[instance.content]: 识别来源 | 进入内容标签时自动 | 按 SHA-1 查 Modrinth：图标、项目名、作者、版本、项目链接；离线时照常列出，未识别的没有切换版本键 | 识别结果不缓存
    pub(in super::super) fn content_panel(
        &mut self,
        window: &mut Window,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let index = self.content_index();
        let kind = CONTENT_KINDS[index];
        let noun = CONTENT_LABELS[index];
        let busy = self.busy;
        let view = cx.entity().downgrade();

        // ia[instance.content]: 切换子分类 | L4a 分段：Mod / 资源包 / 光影 | 列表切换；搜索与筛选各分类分别记住
        let segments = kit::segments("instance-content-kinds", &CONTENT_LABELS, index, {
            let view = view.clone();
            move |index: usize, window: &mut Window, cx: &mut App| {
                let _ = view.update(cx, |view, cx| {
                    view.content_kind = index;
                    view.selected.clear();
                    view.ensure(Section::Content(CONTENT_KINDS[index.min(2)]), window, cx);
                    cx.notify();
                });
            }
        });
        let add = {
            let handler = self.handler.clone();
            // ia[instance.content]: 添加本地文件 | L4a 次要「添加文件」→ 选文件（拖入内容页没做） | 冲突逐项报告，不静默覆盖同名异内容的文件
            kit::action(
                "content-add-files",
                tr!("instance-content-add-files"),
                Some(UiIcon::Plus),
                false,
                move |window, cx| handler(InstanceIntent::ImportContent(kind), window, cx),
            )
            .disabled(busy)
        };
        let browse = {
            let handler = self.handler.clone();
            kit::action(
                "content-browse",
                browse_labels()[index],
                Some(UiIcon::Search),
                true,
                move |window, cx| handler(InstanceIntent::BrowseContent(kind), window, cx),
            )
        };
        let region = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .child(segments)
            .child(h_flex().gap_2().child(add).child(browse));

        let body = if let Some(status) = self.status(
            &self.data.content[index],
            colors,
            Section::Content(kind),
            cx,
        ) {
            status
        } else {
            let filter_select = self.ensure_content_filter_select(window, cx);
            self.content_body(noun, &filter_select, colors, cx)
        };

        v_flex()
            .w_full()
            .gap_4()
            .child(region)
            .child(body)
            .into_any_element()
    }

    /// The Content tab's filter dropdown, created on first use (design
    /// language §10: a Select for picking one value).
    fn ensure_content_filter_select(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> super::FilterSelect {
        if let Some(select) = &self.content_filter_select {
            return select.clone();
        }
        let labels: Vec<String> = filter_labels()
            .iter()
            .map(|label| (*label).to_owned())
            .collect();
        let select = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(labels),
                Some(IndexPath::default()),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &select,
            window,
            |this, _, event: &SelectEvent<SearchableVec<String>>, _, cx| {
                if let SelectEvent::Confirm(Some(label)) = event
                    && let Some(index) = filter_labels().iter().position(|text| *text == label)
                {
                    this.content_filter = FILTERS[index];
                    cx.notify();
                }
            },
        )
        .detach();
        self.content_filter_select = Some(select.clone());
        select
    }

    pub(super) fn content_body(
        &self,
        noun: &str,
        filter_select: &super::FilterSelect,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(list) = self.content_list() else {
            return div().into_any_element();
        };
        if list.entries.is_empty() {
            return kit::empty(
                tr!("instance-content-empty", noun = noun),
                tr!("instance-content-empty-help"),
                colors,
            )
            .into_any_element();
        }
        let busy = self.busy;
        let kind = CONTENT_KINDS[self.content_index()];
        let view = cx.entity().downgrade();
        let query = self.content_query(cx);
        let shown = visible(&list.entries, &query, self.content_filter);
        let updates: Vec<(String, String, String)> = list
            .entries
            .iter()
            .filter_map(|entry| {
                let source = entry.source.as_ref()?;
                let newest = entry.update.as_ref()?;
                Some((
                    entry.item.file_name.clone(),
                    source.project_id.clone(),
                    newest.id.clone(),
                ))
            })
            .collect();

        let toolbar = if self.selected.is_empty() {
            // ia[instance.content]: 筛选 | L4b 下拉：全部 / 有更新 / 已停用 / 未识别 | 视图状态
            let filters = div().w(px(140.)).child(Select::new(filter_select).small());
            let update_all = (!updates.is_empty()).then(|| {
                let view = view.clone();
                let count = updates.len();
                // ia[instance.content]: 全部更新 | L4b「全部更新（N）」→ 确认弹窗 | 逐项执行，部分成功分别报告；进度在动态 | 弹窗内逐项取消勾选没做
                let updates = updates.clone();
                theme::clickable(
                    Key::new("content-update-all")
                        .icon(Icon::new(UiIcon::Refresh))
                        .label(tr!("instance-content-update-all-count", count = count))
                        .white()
                        .small()
                        .disabled(busy),
                    !busy,
                )
                .on_click(move |_, window, cx| {
                    let updates = updates.clone();
                    let _ = view.update(cx, |view, cx| {
                        view.confirm_update_all(kind, updates, window, cx)
                    });
                })
            });
            let refresh = {
                // ia[instance.content]: 刷新 | L4b ↻ | 重新扫描目录并重新识别
                let view = view.clone();
                theme::clickable(
                    Key::new("content-refresh")
                        .icon(Icon::new(UiIcon::Refresh))
                        .ghost()
                        .small()
                        .tooltip(tr!("instance-content-refresh-tip")),
                    true,
                )
                .on_click(move |_, window, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.request(Section::Content(kind), window, cx)
                    });
                })
            };
            let search = self.fields.as_ref().map(|fields| {
                div().w(px(220.)).child(
                    Input::new(&fields.content_search).small().prefix(
                        Icon::new(UiIcon::Search)
                            .size(px(14.))
                            .text_color(colors.muted),
                    ),
                )
            });
            // Same height as the bulk bar that replaces it, so selecting a
            // row never moves the list under the pointer.
            h_flex()
                .w_full()
                .h(BAR_HEIGHT)
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .children(search)
                        .child(filters),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .children(update_all)
                        .child(refresh),
                )
                .into_any_element()
        } else {
            self.bulk_bar(colors, cx)
        };

        let rows: Vec<AnyElement> = shown
            .iter()
            .enumerate()
            .map(|(index, entry)| self.row(index, entry, colors, cx))
            .collect();
        let list_block = if rows.is_empty() {
            kit::empty(
                tr!("instance-content-no-match"),
                tr!("instance-content-no-match-help"),
                colors,
            )
            .into_any_element()
        } else {
            kit::panel_list(rows, colors).into_any_element()
        };
        v_flex()
            .w_full()
            .gap_3()
            .child(toolbar)
            .child(list_block)
            .when(list.sources_unavailable, |body| {
                let reason = list
                    .source_note
                    .as_deref()
                    .map_or_else(String::new, |note| format!("（{note}）"));
                body.child(
                    div()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(tr!("instance-content-source-note", reason = reason)),
                )
            })
            .into_any_element()
    }

    /// Wires the content search field: typing refilters the list.
    // ia[instance.content]: 搜索 | L4b 搜索框 | 按名称过滤（视图状态）
    pub(in super::super) fn watch_content_search(
        search: &Entity<InputState>,
        cx: &mut Context<Self>,
    ) {
        cx.subscribe(search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
    }
}
