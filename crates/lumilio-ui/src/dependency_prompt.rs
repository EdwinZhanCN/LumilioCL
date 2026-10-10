//! The dependency prompt of Discover: before a mod is installed, the required
//! mods the game does not have yet, all ticked. It holds no business logic: the
//! answer is the projects to install along.

use std::rc::Rc;

use crate::controls::Checkbox;
use crate::key::Key;
use gpui::{App, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};
use gpui_component::{WindowExt as _, h_flex, v_flex};
use lumilio_core::{DependencyNeed, DependencyReport};

use crate::theme::{self, ShellColors};
use crate::tr;

const DIALOG_WIDTH: f32 = 420.;

/// The projects (by id) to install together with the mod.
pub type DependencyHandler = Rc<dyn Fn(Vec<String>, &mut Window, &mut App)>;

/// The projects ticked, in the order they are listed. A dependency with no
/// version for this game can never be ticked.
#[must_use]
pub fn chosen(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter(|(need, on, _)| *on && need.version.is_some())
        .map(|(need, _, _)| need.project_id.clone())
        .collect()
}

/// A mod to install along, whether it is ticked, and whether it is only a
/// suggestion.
pub type Row = (DependencyNeed, bool, bool);

pub struct DependencyPrompt {
    mod_title: String,
    rows: Vec<Row>,
    conflicts: Vec<String>,
    handler: DependencyHandler,
}

impl DependencyPrompt {
    /// Required ones start ticked (when there is a version for this game),
    /// suggestions start unticked.
    pub fn new(mod_title: String, report: DependencyReport, handler: DependencyHandler) -> Self {
        let rows = report
            .needs
            .into_iter()
            .map(|need| {
                let on = need.version.is_some();
                (need, on, false)
            })
            .chain(report.optional.into_iter().map(|need| (need, false, true)))
            .collect();
        Self {
            mod_title,
            rows,
            conflicts: report.conflicts,
            handler,
        }
    }

    /// Whether there is anything to ask or warn about.
    #[must_use]
    pub fn worth_showing(report: &DependencyReport) -> bool {
        !report.needs.is_empty() || !report.optional.is_empty() || !report.conflicts.is_empty()
    }

    fn answer(&self, together: bool, window: &mut Window, cx: &mut App) {
        let projects = if together {
            chosen(&self.rows)
        } else {
            Vec::new()
        };
        (self.handler)(projects, window, cx);
        window.close_dialog(cx);
    }

    // ia[discover]: 依赖提示 | 安装 Mod 前的弹窗 | 列出目标游戏还缺的必需依赖（默认勾选；没有适合版本的标明且不能勾；「只安装它」跳过）；也列可选依赖（默认不勾）并警告与已装 Mod 不兼容；依赖先装，每个是动态里的一条任务 | 仅 Mod
    pub fn open(form: Entity<Self>, window: &mut Window, cx: &mut App) {
        window.open_dialog(cx, move |dialog, _, cx| {
            let this = form.read(cx);
            let count = chosen(&this.rows).len();
            let mod_title = this.mod_title.clone();
            let required_any = this.rows.iter().any(|(_, _, optional)| !optional);
            let (together, alone) = (form.downgrade(), form.downgrade());
            let go = theme::clickable(
                Key::new("dependency-together")
                    .label(if count == 0 {
                        tr!("dependency-install-alone").to_owned()
                    } else {
                        tr!("dependency-install-together", count = count + 1)
                    })
                    .primary()
                    .debug_selector(|| "dependency-together".into())
                    .on_click(move |_, window, cx| {
                        let _ = together.update(cx, |form, cx| form.answer(true, window, cx));
                    }),
                true,
            );
            theme::dialog(dialog, cx)
                .title(if required_any {
                    tr!("dependency-needs-more", mod_title = mod_title.as_str())
                } else {
                    tr!("dependency-about", mod_title = mod_title.as_str())
                })
                .w(px(DIALOG_WIDTH))
                .child(form.clone())
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Key::new("dependency-cancel")
                                .label(tr!("common-cancel"))
                                .white()
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .children((count > 0).then(|| {
                            Key::new("dependency-alone")
                                .label(tr!("dependency-install-alone"))
                                .white()
                                .debug_selector(|| "dependency-alone".into())
                                .on_click(move |_, window, cx| {
                                    let _ =
                                        alone.update(cx, |form, cx| form.answer(false, window, cx));
                                })
                        }))
                        .child(go),
                )
        });
    }
}

impl Render for DependencyPrompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ShellColors::current(cx);
        let weak = cx.entity().downgrade();
        let row = |index: usize, need: &DependencyNeed, on: bool| {
            let weak = weak.clone();
            let available = need.version.is_some();
            let label = match &need.version {
                Some(version) => format!("{} {}", need.title, version.number),
                None => tr!("dependency-no-version", title = need.title.as_str()),
            };
            Checkbox::new(("dependency-row", index))
                .checked(on && available)
                .disabled(!available)
                .label(label)
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |prompt, cx| {
                        if let Some(row) = prompt.rows.get_mut(index) {
                            row.1 = !row.1;
                        }
                        cx.notify();
                    });
                })
        };
        let section = |optional: bool, header: &'static str| {
            let rows: Vec<_> = self
                .rows
                .iter()
                .enumerate()
                .filter(|(_, (_, _, is_optional))| *is_optional == optional)
                .map(|(index, (need, on, _))| row(index, need, *on))
                .collect();
            (!rows.is_empty()).then(|| {
                v_flex()
                    .gap_2()
                    .child(div().text_sm().text_color(colors.muted).child(header))
                    .children(rows)
            })
        };
        v_flex()
            .w_full()
            .gap_4()
            .children(section(false, tr!("dependency-needed")))
            .children(section(true, tr!("dependency-optional")))
            .children((!self.conflicts.is_empty()).then(|| {
                div()
                    .debug_selector(|| "dependency-conflicts".into())
                    .text_sm()
                    .text_color(colors.danger)
                    .child(tr!(
                        "dependency-conflicts",
                        conflicts = self.conflicts.join(tr!("common-list-separator"))
                    ))
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumilio_core::{ReleaseChannel, Version};

    fn version() -> Version {
        Version {
            id: "v".into(),
            project_id: "p".into(),
            name: "n".into(),
            number: "1.2".into(),
            channel: ReleaseChannel::Release,
            game_versions: Vec::new(),
            loaders: Vec::new(),
            published: String::new(),
            files: Vec::new(),
            dependencies: Vec::new(),
            downloads: 0,
            changelog: String::new(),
        }
    }

    fn need(id: &str, fits: bool) -> DependencyNeed {
        DependencyNeed {
            project_id: id.into(),
            title: id.to_uppercase(),
            version: fits.then(version),
        }
    }

    #[test]
    fn everything_installable_starts_ticked_and_an_unfit_one_can_never_be_chosen() {
        let prompt = DependencyPrompt::new(
            "Cool".into(),
            DependencyReport {
                needs: vec![need("a", true), need("b", false), need("c", true)],
                optional: vec![need("o", true)],
                conflicts: vec!["X".into()],
            },
            Rc::new(|_, _, _| {}),
        );
        assert_eq!(
            chosen(&prompt.rows),
            ["a", "c"],
            "suggestions start unticked"
        );
        let mut rows = prompt.rows;
        rows[0].1 = false;
        rows[1].1 = true;
        rows[3].1 = true;
        assert_eq!(
            chosen(&rows),
            ["c", "o"],
            "unticked and unfit ones stay out"
        );
        assert!(DependencyPrompt::worth_showing(&DependencyReport {
            conflicts: vec!["X".into()],
            ..DependencyReport::default()
        }));
        assert!(!DependencyPrompt::worth_showing(
            &DependencyReport::default()
        ));
    }
}
