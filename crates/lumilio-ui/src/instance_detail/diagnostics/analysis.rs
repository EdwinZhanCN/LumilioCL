use super::super::{InstanceDetailView, InstanceIntent};
use crate::{
    kit,
    theme::{self, ShellColors},
    tr,
};
use gpui::prelude::*;
use gpui::{App, Context, Entity, Window, div, px};
use gpui_component::{WindowExt as _, dialog::Dialog, h_flex, v_flex};
use lumilio_core::GameLogSource;

impl InstanceDetailView {
    pub(super) fn open_log_analysis(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = self.log_text().map(|text| text.into_owned()) else {
            return;
        };
        self.analysis_serial += 1;
        let request = self.analysis_serial;
        self.log_analysis = Some((request, self.log_source_label(), None));
        let weak = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| match weak.upgrade() {
            Some(view) => Self::log_analysis_dialog(&view, dialog, cx),
            None => dialog,
        });
        (self.handler)(
            InstanceIntent::AnalyzeGameLog {
                request,
                text,
                crash: matches!(self.log_source, GameLogSource::Crash(_)),
            },
            window,
            cx,
        );
        cx.notify();
    }

    fn log_analysis_dialog(view: &Entity<Self>, dialog: Dialog, cx: &mut App) -> Dialog {
        let this = view.read(cx);
        let Some((_, source, read)) = &this.log_analysis else {
            return dialog;
        };
        let colors = ShellColors::current(cx);
        let mut body = v_flex().gap_3().child(
            div()
                .text_sm()
                .text_color(colors.muted)
                .child(source.clone()),
        );
        let mut technical = None;
        match read {
            None => {
                body = body.child(div().child(tr!("instance-analysis-running")));
            }
            Some(Err(detail)) => {
                body = body
                    .child(div().child(tr!("instance-analysis-failed")))
                    .child(technical_detail("log-analysis-error", detail, colors));
            }
            Some(Ok((text, findings))) => {
                let mut details = tr!("instance-analysis-source", source = source.as_str());
                details.push('\n');
                if findings.is_empty() {
                    body = body
                        .child(
                            div()
                                .debug_selector(|| "log-analysis-empty".into())
                                .child(tr!("instance-analysis-none")),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(colors.muted)
                                .child(tr!("instance-analysis-none-help")),
                        );
                    details.push_str(tr!("instance-analysis-none"));
                    details.push('\n');
                }
                for (index, result) in findings.iter().enumerate() {
                    let finding = &result.finding;
                    details.push_str(&format!("\n{}\n{}\n", finding.title, finding.advice));
                    if let Some(evidence) = &finding.evidence {
                        details.push_str(evidence);
                        details.push('\n');
                    }
                    body = body.child(
                        v_flex()
                            .gap_2()
                            .debug_selector(move || format!("crash-finding-{index}"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.foreground)
                                    .child(finding.title.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.muted)
                                    .child(finding.advice.clone()),
                            )
                            .when_some(finding.evidence.as_ref(), |view, evidence| {
                                view.child(technical_detail(
                                    format!("log-analysis-evidence-{index}"),
                                    evidence,
                                    colors,
                                ))
                            }),
                    );
                }
                details.push('\n');
                details.push_str(tr!("instance-analysis-snapshot"));
                details.push('\n');
                details.push_str(text);
                technical = Some(details);
            }
        }
        let closed = view.downgrade();
        // ia[instance.diagnostics]: 复制分析技术详情 | 崩溃分析弹窗 · 按键「复制技术详情」 | 复制来源、分析结果、证据与点击时的完整日志快照，内容已脱敏；加载和失败时禁用
        let copy = kit::ghost("log-analysis-copy", tr!("instance-analysis-copy"), {
            let technical = technical.clone();
            move |_, cx| {
                if let Some(text) = &technical {
                    crate::toast::copy_text(text.clone(), cx);
                }
            }
        })
        .disabled(technical.is_none())
        .debug_selector(|| "log-analysis-copy".into());
        dialog
            .title(tr!("instance-analysis-title"))
            .w(px(600.))
            .on_close(move |_, _, cx| {
                let _ = closed.update(cx, |view, cx| {
                    view.log_analysis = None;
                    cx.notify();
                });
            })
            .child(
                div()
                    .id("log-analysis-content")
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .child(body),
            )
            .footer(h_flex().w_full().justify_end().child(copy))
    }
}

/// Analysis is already a reading surface; show its evidence and errors here
/// instead of stacking a second dialog above it.
fn technical_detail(
    selector: impl Into<gpui::SharedString>,
    detail: &str,
    colors: ShellColors,
) -> impl IntoElement {
    let selector = selector.into();
    v_flex()
        .w_full()
        .gap_2()
        .child(kit::section_label(tr!("common-technical-details"), colors))
        .child(
            div()
                .w_full()
                .min_w_0()
                .debug_selector(move || selector.to_string())
                .font_family(theme::mono_font())
                .text_xs()
                .text_color(colors.muted)
                .child(detail.to_owned()),
        )
}
