//! The Java group's first row: a dropdown of 自动（推荐）, every Java found
//! and 自定义路径…, with what the next launch uses beneath it. The saved value
//! stays `java_path` (`None` is automatic).

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Window, div, px};
use gpui_component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_component::{IndexPath, Sizable as _};
use lumilio_core::{HostProfile, InstanceJava, InstanceSettings};

use super::super::{Dropdown, InstanceDetailView, Section};
use super::saves::{cleared, java};
use crate::live::java_label;
use crate::settings_dialog::{DialogSpec, FieldKind, FieldSpec, SettingsDialog};
use crate::theme::ShellColors;
use crate::tr;

/// One entry of the Java dropdown.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JavaPick {
    Automatic,
    /// A Java found on this machine, by its home folder.
    Installed(PathBuf),
    /// A path typed or chosen in the path dialog.
    Custom,
}

/// The dropdown's entries with their labels. Two installations that read the
/// same get their folder name added so every label is distinct.
#[must_use]
pub fn java_picks(java: Option<&InstanceJava>, host: &HostProfile) -> Vec<(JavaPick, String)> {
    let mut picks = vec![(
        JavaPick::Automatic,
        tr!("instance-settings-java-automatic").to_owned(),
    )];
    let runtimes = java.map_or(&[][..], |java| java.runtimes.as_slice());
    let labels: Vec<String> = runtimes
        .iter()
        .map(|runtime| java_label(runtime, host))
        .collect();
    for (runtime, label) in runtimes.iter().zip(&labels) {
        let label = if labels.iter().filter(|other| *other == label).count() > 1 {
            let folder = runtime
                .home()
                .ancestors()
                .find(|part| {
                    part.file_name()
                        .is_some_and(|name| name != "Home" && name != "Contents")
                })
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            format!("{label} · {folder}")
        } else {
            label.clone()
        };
        picks.push((JavaPick::Installed(runtime.home().to_owned()), label));
    }
    picks.push((
        JavaPick::Custom,
        tr!("instance-settings-java-custom").to_owned(),
    ));
    picks
}

/// The entry the saved path stands for: automatic when none is saved, an
/// installation when the path is its home or executable, else the custom one.
#[must_use]
pub fn picked(java: Option<&InstanceJava>, java_path: Option<&Path>) -> JavaPick {
    let Some(path) = java_path else {
        return JavaPick::Automatic;
    };
    java.and_then(|java| {
        java.runtimes
            .iter()
            .find(|runtime| runtime.home() == path || runtime.executable() == path)
    })
    .map_or(JavaPick::Custom, |runtime| {
        JavaPick::Installed(runtime.home().to_owned())
    })
}

/// What the next launch does, under the dropdown.
#[must_use]
pub fn java_summary(java: Option<&InstanceJava>) -> String {
    let Some(java) = java else {
        return tr!("instance-settings-java-reading").to_owned();
    };
    match (&java.choice, java.needed) {
        (Some(choice), _) => tr!(
            "instance-settings-java-uses",
            java = format!("Java {}", choice.runtime.version()),
            path = choice.runtime.home().display().to_string()
        ),
        (None, Some(major)) if java.auto_install => {
            tr!("instance-settings-java-will-download", major = major)
        }
        (None, None) if java.auto_install => {
            tr!("instance-settings-java-will-download-any").to_owned()
        }
        (None, Some(major)) => tr!("instance-settings-java-none-fits", major = major),
        (None, None) => tr!("instance-problem-no-java").to_owned(),
    }
}

/// The dialog for a path of one's own: a java program or a JDK folder.
fn path_spec(current: &InstanceSettings) -> DialogSpec<super::super::InstanceIntent> {
    let current = current.clone();
    DialogSpec {
        title: "Java",
        intro: Some(tr!("instance-settings-java-intro")),
        fields: vec![FieldSpec {
            label: tr!("instance-settings-path"),
            help: None,
            placeholder: tr!("instance-settings-java-path-placeholder"),
            value: current
                .java_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            kind: FieldKind::Path {
                files: true,
                directories: true,
                prompt: tr!("instance-settings-java-pick"),
            },
        }],
        parse: {
            let current = current.clone();
            Rc::new(move |values| {
                if values[0].trim().is_empty() {
                    return Err(tr!("instance-settings-java-path-empty").to_owned());
                }
                java(&current, &values[0])
            })
        },
        reset: cleared(&current, |next| next.java_path = None),
    }
}

impl InstanceDetailView {
    /// The dropdown, created on first use and kept in step with the Java
    /// found and the saved choice.
    fn java_select(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Dropdown {
        let host = HostProfile::current();
        let java = self.data.java.as_ref().and_then(|java| java.as_ref().ok());
        let picks = java_picks(java, &host);
        let saved = self
            .record
            .as_ref()
            .and_then(|record| record.settings.java_path.clone());
        let current = picked(java, saved.as_deref());
        let selected_label = picks
            .iter()
            .find(|(pick, _)| *pick == current)
            .map(|(_, label)| label.clone());
        let labels: Vec<String> = picks.iter().map(|(_, label)| label.clone()).collect();
        let select = if let Some(select) = &self.java_select {
            select.clone()
        } else {
            let selected = picks
                .iter()
                .position(|(pick, _)| *pick == current)
                .map(IndexPath::new);
            let select = cx.new(|cx| {
                SelectState::new(SearchableVec::new(labels.clone()), selected, window, cx)
                    .searchable(true)
            });
            cx.subscribe_in(
                &select,
                window,
                |view, _, event: &SelectEvent<SearchableVec<String>>, window, cx| {
                    if let SelectEvent::Confirm(Some(label)) = event
                        && let Some((pick, _)) =
                            view.java_picks.iter().find(|(_, text)| text == label)
                    {
                        view.pick_java(pick.clone(), window, cx);
                    }
                },
            )
            .detach();
            self.java_select = Some(select.clone());
            self.java_picks = picks.clone();
            select
        };
        if self.java_picks != picks {
            select.update(cx, |select, cx| {
                select.set_items(SearchableVec::new(labels), window, cx);
            });
            self.java_picks = picks;
        }
        if select.read(cx).selected_value() != selected_label.as_ref()
            && let Some(label) = &selected_label
        {
            select.update(cx, |select, cx| {
                select.set_selected_value(label, window, cx)
            });
        }
        select
    }

    /// Saves what was chosen; 自定义路径… asks for the path first.
    fn pick_java(&mut self, pick: JavaPick, window: &mut Window, cx: &mut Context<Self>) {
        let Some(record) = &self.record else {
            return;
        };
        let current = record.settings.clone();
        let next = match pick {
            JavaPick::Automatic => None,
            JavaPick::Installed(home) => Some(home),
            JavaPick::Custom => {
                SettingsDialog::open(path_spec(&current), self.handler.clone(), window, cx);
                // Until a path is saved the dropdown shows what is saved.
                cx.notify();
                return;
            }
        };
        if next != current.java_path
            && let Ok(intent) = java(
                &current,
                &next
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
            )
        {
            (self.handler)(intent, window, cx);
        }
    }

    pub(super) fn java_row(
        &mut self,
        window: &mut Window,
        colors: ShellColors,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::Stateful<gpui::Div>> {
        self.ensure(Section::Java, window, cx);
        let select = self.java_select(window, cx);
        let java = self.data.java.as_ref().and_then(|java| java.as_ref().ok());
        let record = self.record.as_ref().expect("loaded");
        let custom = picked(java, record.settings.java_path.as_deref()) == JavaPick::Custom;
        let busy = self.busy;
        // ia[instance.settings]: 指定 Java | 设置 · Java 组，下拉「自动（推荐）/ 已发现的 Java / 自定义路径…」 | 选中即保存；值下方写出下次启动用哪个 Java，没有合适的时写出启动时会下载的版本；已停用的 Java 不在列表里 | java-management T4
        let mut rows = vec![
            self.setting_row(
                "isettings-java",
                "Java",
                Some(tr!("instance-settings-java-help").to_owned()),
                java_summary(java),
                Some(
                    div()
                        .w(px(300.))
                        .child(
                            Select::new(&select)
                                .small()
                                .accessibility_label("Java")
                                .menu_width(px(380.))
                                .search_placeholder(tr!("instance-settings-java-search"))
                                .disabled(busy),
                        )
                        .into_any_element(),
                ),
                colors,
            ),
        ];
        if custom {
            let current = record.settings.clone();
            // ia[instance.settings]: 自定义 Java 路径 | 设置 · Java 组，选「自定义路径…」后出现的路径行 + [编辑] 弹窗 | 填 java 程序或 JDK 文件夹；保存时检查路径存在；恢复默认＝回到自动
            rows.push(
                self.setting_row(
                    "isettings-java-path",
                    tr!("instance-settings-path"),
                    None,
                    current
                        .java_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_default(),
                    Some(self.edit_spec("isettings-java-edit", move || path_spec(&current))),
                    colors,
                ),
            );
        }
        rows
    }
}
