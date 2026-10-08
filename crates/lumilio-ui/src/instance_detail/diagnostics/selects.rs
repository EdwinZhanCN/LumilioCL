use super::super::{Dropdown, InstanceDetailView};
use crate::tr;
use gpui::{AppContext as _, Context, Window};
use gpui_component::{
    IndexPath,
    select::{SearchableVec, SelectEvent, SelectState},
};
use lumilio_core::{GameLogSource, LogLevel};

pub(super) fn levels() -> [(LogLevel, &'static str); 4] {
    [
        (LogLevel::Error, tr!("instance-log-level-error")),
        (LogLevel::Warn, tr!("instance-log-level-warn")),
        (LogLevel::Info, tr!("instance-log-level-info")),
        (LogLevel::Debug, tr!("instance-log-level-debug")),
    ]
}

impl InstanceDetailView {
    pub(super) fn log_selects(
        &mut self,
        choices: Vec<(GameLogSource, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Dropdown, Dropdown) {
        let labels = choices
            .iter()
            .map(|(_, label)| label.clone())
            .collect::<Vec<_>>();
        let selected = choices
            .iter()
            .position(|(source, _)| *source == self.log_source)
            .map(IndexPath::new);
        let selected_label = choices
            .iter()
            .find(|(source, _)| *source == self.log_source)
            .map(|(_, label)| label.clone());
        let source = if let Some(select) = &self.log_source_select {
            select.clone()
        } else {
            let select = cx.new(|cx| {
                SelectState::new(SearchableVec::new(labels.clone()), selected, window, cx)
                    .searchable(true)
            });
            cx.subscribe_in(
                &select,
                window,
                |view, _, event: &SelectEvent<SearchableVec<String>>, window, cx| {
                    if let SelectEvent::Confirm(Some(label)) = event
                        && let Some((source, _)) =
                            view.log_choices.iter().find(|(_, text)| text == label)
                    {
                        view.choose_log(source.clone(), window, cx);
                    }
                },
            )
            .detach();
            self.log_source_select = Some(select.clone());
            select
        };
        if self.log_choices != choices {
            source.update(cx, |select, cx| {
                select.set_items(SearchableVec::new(labels), window, cx);
                if let Some(label) = &selected_label {
                    select.set_selected_value(label, window, cx);
                } else {
                    select.set_selected_index(None, window, cx);
                }
            });
            self.log_choices = choices;
        } else if source.read(cx).selected_value() != selected_label.as_ref() {
            source.update(cx, |select, cx| {
                if let Some(label) = &selected_label {
                    select.set_selected_value(label, window, cx);
                } else {
                    select.set_selected_index(None, window, cx);
                }
            });
        }

        let level_select = if let Some(select) = &self.log_level_select {
            select.clone()
        } else {
            let indices = levels()
                .iter()
                .enumerate()
                .filter(|(_, (level, _))| self.log_levels.contains(level))
                .map(|(ix, _)| IndexPath::new(ix))
                .collect();
            let select = cx.new(|cx| {
                SelectState::new_multiple(
                    SearchableVec::new(
                        levels()
                            .iter()
                            .map(|(_, label)| label.to_string())
                            .collect::<Vec<_>>(),
                    ),
                    indices,
                    window,
                    cx,
                )
            });
            cx.subscribe_in(
                &select,
                window,
                |view, _, event: &SelectEvent<SearchableVec<String>>, _, cx| {
                    if let SelectEvent::Change(labels) = event {
                        view.log_levels = levels()
                            .iter()
                            .filter(|(_, label)| labels.iter().any(|text| text == label))
                            .map(|(level, _)| *level)
                            .collect();
                        cx.notify();
                    }
                },
            )
            .detach();
            self.log_level_select = Some(select.clone());
            select
        };
        let values = levels()
            .iter()
            .filter(|(level, _)| self.log_levels.contains(level))
            .map(|(_, label)| label.to_string())
            .collect::<Vec<_>>();
        let committed = level_select.read(cx).selected_values();
        if values.len() != committed.len() || values.iter().any(|value| !committed.contains(value))
        {
            level_select.update(cx, |select, cx| {
                select.set_selected_values(&values, window, cx)
            });
        }
        (source, level_select)
    }
}
