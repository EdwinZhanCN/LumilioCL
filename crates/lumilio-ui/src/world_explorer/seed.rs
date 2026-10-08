use super::{Command, MapView};
use crate::{
    settings_dialog::{DialogSpec, FieldKind, FieldSpec, SettingsDialog},
    tr,
};
use gpui::{Context, Window};
use std::rc::Rc;

/// Kept in sync with the provider by an application assembly test.
pub const VERSIONS: &[&str] = &[
    "1.0.0", "1.1", "1.2.5", "1.3.2", "1.4.7", "1.5.2", "1.6.4", "1.7.10", "1.8.9", "1.9.4",
    "1.10.2", "1.11.2", "1.12.2", "1.13.2", "1.14.4", "1.15.2", "1.16.1", "1.16.5", "1.17.1",
    "1.18.2", "1.19.2", "1.19.4", "1.20.6", "1.21.1", "1.21.3", "1.21.4",
];
impl MapView {
    pub(super) fn manual_seed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = cx.weak_entity();
        let selected = self
            .context
            .as_ref()
            .and_then(|context| context.version.as_deref())
            .and_then(|name| VERSIONS.iter().position(|candidate| *candidate == name))
            .unwrap_or(VERSIONS.len() - 1);
        SettingsDialog::open(
            DialogSpec {
                title: tr!("map-enter-seed"),
                intro: Some(tr!("map-manual-help")),
                fields: vec![
                    FieldSpec {
                        label: tr!("map-seed"),
                        help: None,
                        placeholder: tr!("map-seed-placeholder"),
                        value: self
                            .context
                            .as_ref()
                            .and_then(|context| context.seed)
                            .map(|seed| seed.to_string())
                            .unwrap_or_default(),
                        kind: FieldKind::Line,
                    },
                    FieldSpec {
                        label: tr!("map-version"),
                        help: None,
                        placeholder: "",
                        value: selected.to_string(),
                        kind: FieldKind::Choice {
                            labels: VERSIONS,
                            selected,
                        },
                    },
                ],
                parse: Rc::new(|values| {
                    let seed = lumilio_core::world_map::store::parse_seed(&values[0])
                        .ok_or_else(|| tr!("map-seed-empty").to_owned())?;
                    let index = values[1]
                        .parse::<usize>()
                        .map_err(|_| tr!("map-version-unsupported").to_owned())?;
                    let version = VERSIONS
                        .get(index)
                        .ok_or_else(|| tr!("map-version-unsupported").to_owned())?;
                    Ok((seed, version.to_string()))
                }),
                reset: None,
            },
            Rc::new(move |(seed, version), _, cx| {
                let _ = target.update(cx, |this, cx| {
                    if let Some(connection) = &this.connection
                        && let Err(error) = connection
                            .send
                            .try_send(Command::SaveSeed { seed, version })
                    {
                        this.error = Some(error.to_string());
                    }
                    cx.notify();
                });
            }),
            window,
            cx,
        );
    }
}
