//! Translation on a project page (plan discover-translation): MCIM's
//! Chinese summary in the header, and the body translated on request by the
//! provider chosen in Settings. The title is never translated.

use super::{DetailIntent, DetailState, ProjectDetailView};
use crate::assets::UiIcon;
use crate::key::{Key, KeyKind};
use crate::tr;
use gpui::prelude::*;
use gpui::{Context, Window};
use gpui_component::{Icon, Selectable as _};
use lumilio_core::{Project, TextFormat, TranslatedDocument, TranslationError};

/// What the page knows about translation. Every change of project or of the
/// setting moves `revision` and `request` on, so an answer that arrives
/// late is dropped instead of shown.
#[derive(Default)]
pub(super) struct DetailTranslation {
    /// Translation is on in Settings.
    enabled: bool,
    /// MCIM's summary for the project shown, when it has one.
    summary: Option<String>,
    revision: u64,
    /// The body request the page waits for, while `loading`.
    request: u64,
    loading: bool,
    body: Option<TranslatedDocument>,
    /// The last request failed; the key offers to retry.
    failed: bool,
    /// The body shows the translation rather than the original.
    shown: bool,
}

/// The summary the person reads, and what to ask MCIM for it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SummaryQuery {
    pub source: String,
    pub project: String,
    pub original: String,
    pub revision: u64,
}

impl DetailTranslation {
    /// Forgets everything about the project shown: a new project, or the
    /// setting turned off, starts from the original.
    fn reset(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.summary = None;
        self.request = self.request.wrapping_add(1);
        self.loading = false;
        self.body = None;
        self.failed = false;
        self.shown = false;
    }
}

/// The text a body request sends: the body, or the summary when the source
/// gave no body (which is then plain text).
fn body_original(project: &Project) -> (String, TextFormat) {
    if project.body.trim().is_empty() {
        (project.description.clone(), TextFormat::Markdown)
    } else {
        (project.body.clone(), project.body_format)
    }
}

impl ProjectDetailView {
    pub(super) fn forget_translation(&mut self) {
        self.translation.reset();
    }

    pub fn set_translation_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.translation.enabled != enabled {
            self.translation.reset();
            self.translation.enabled = enabled;
            cx.notify();
        }
    }

    /// The body request still wanted, if one is running.
    pub fn translation_request(&self) -> Option<u64> {
        (self.translation.enabled && self.translation.loading).then_some(self.translation.request)
    }

    /// What to ask MCIM for the header's summary; `None` while translation
    /// is off or the project has not loaded.
    pub fn summary_query(&self) -> Option<SummaryQuery> {
        if !self.translation.enabled {
            return None;
        }
        let DetailState::Ready(detail) = &self.state else {
            return None;
        };
        Some(SummaryQuery {
            source: detail.source.clone(),
            project: detail.project.id.clone(),
            original: detail.project.description.clone(),
            revision: self.translation.revision,
        })
    }

    /// MCIM's answer for `asked`; dropped when the page has moved on since.
    pub fn set_summary_translation(
        &mut self,
        asked: &SummaryQuery,
        translated: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.summary_query().as_ref() != Some(asked) {
            return;
        }
        self.translation.summary = translated;
        cx.notify();
    }

    pub fn set_translation(
        &mut self,
        request: u64,
        result: Result<TranslatedDocument, TranslationError>,
        cx: &mut Context<Self>,
    ) {
        if self.translation_request() != Some(request) {
            return;
        }
        let translation = &mut self.translation;
        translation.loading = false;
        match result {
            Ok(body) => {
                translation.body = Some(body);
                translation.shown = true;
                translation.failed = false;
            }
            Err(TranslationError::Disabled) => {
                translation.failed = false;
                translation.shown = false;
            }
            Err(error) => {
                translation.failed = true;
                translation.shown = false;
                self.toasts.push(
                    crate::toast::Toast::error(tr!("translation-failed"))
                        .technical(error.to_string()),
                );
            }
        }
        cx.notify();
    }

    fn toggle_translation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.translation.enabled || self.translation.loading {
            return;
        }
        if self.translation.shown {
            self.translation.shown = false;
            cx.notify();
            return;
        }
        let DetailState::Ready(detail) = &self.state else {
            return;
        };
        let (original, format) = body_original(&detail.project);
        let translation = &mut self.translation;
        translation.request = translation.request.wrapping_add(1);
        translation.loading = true;
        translation.failed = false;
        (self.handler)(
            DetailIntent::Translate {
                source: detail.source.clone(),
                project: detail.project.id.clone(),
                original,
                format,
                request: translation.request,
            },
            window,
            cx,
        );
        cx.notify();
    }

    /// The header's summary: MCIM's while translation is on, else the source's.
    pub(super) fn summary_text(&self, project: &Project) -> String {
        self.translation
            .summary
            .clone()
            .unwrap_or_else(|| project.description.clone())
    }

    /// The translated body, while the person has chosen to read it.
    pub(super) fn shown_translation(&self) -> Option<&TranslatedDocument> {
        self.translation
            .shown
            .then_some(self.translation.body.as_ref())
            .flatten()
    }

    /// The key left of Install that switches the body between the original
    /// and the translation; none while translation is off.
    pub(super) fn translate_key(&self, cx: &mut Context<Self>) -> Option<Key> {
        let translation = &self.translation;
        if !translation.enabled {
            return None;
        }
        let entity = cx.entity();
        let active = translation.shown || translation.loading;
        // ia[discover]: 翻译 / 显示原文 | 详情页安装键左侧 · Languages 图标键 | 开启与翻译中用主题强调色；再按恢复正文原文；简介自动用 MCIM，没有时用 launcher.lumilio.org，标题不翻译
        Some(
            Key::new("detail-translate")
                .icon(Icon::new(UiIcon::Languages))
                .kind(if active {
                    KeyKind::Orange
                } else {
                    KeyKind::White
                })
                .selected(active)
                .loading(translation.loading)
                .tooltip(if translation.loading {
                    tr!("translation-loading")
                } else if translation.shown {
                    tr!("translation-original")
                } else if translation.failed {
                    tr!("translation-retry")
                } else {
                    tr!("translation-action")
                })
                .debug_selector(|| "detail-translate".into())
                .on_click(move |_, window, cx| {
                    entity.update(cx, |view, cx| view.toggle_translation(window, cx))
                }),
        )
    }
}
