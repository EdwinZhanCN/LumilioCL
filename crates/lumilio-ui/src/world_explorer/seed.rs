use super::{Command, MapView};
use crate::tr;
use gpui::{AppContext as _, Context, Entity, Focusable as _, SharedString, Window};
use gpui_component::{
    IndexPath,
    input::{InputEvent, InputState},
    select::{SearchableVec, SelectEvent, SelectItem, SelectState},
};
use lumilio_plugin_api::map::{WorldContext, WorldId};

/// Kept in sync with the provider by an application assembly test.
pub const VERSIONS: &[&str] = &[
    "1.0.0", "1.1", "1.2.5", "1.3.2", "1.4.7", "1.5.2", "1.6.4", "1.7.10", "1.8.9", "1.9.4",
    "1.10.2", "1.11.2", "1.12.2", "1.13.2", "1.14.4", "1.15.2", "1.16.1", "1.16.5", "1.17.1",
    "1.18.2", "1.19.2", "1.19.4", "1.20.6", "1.21.1", "1.21.3", "1.21.4",
];

#[derive(Clone, PartialEq)]
pub(super) struct WorldChoice {
    id: WorldId,
    name: String,
}
impl SelectItem for WorldChoice {
    type Value = WorldId;
    fn title(&self) -> SharedString {
        self.name.clone().into()
    }
    fn value(&self) -> &WorldId {
        &self.id
    }
}

pub(super) struct Form {
    pub seed: Entity<InputState>,
    pub version: Entity<SelectState<SearchableVec<String>>>,
    pub world: Entity<SelectState<SearchableVec<WorldChoice>>>,
    pub jump_x: Entity<InputState>,
    pub jump_z: Entity<InputState>,
    pub seed_error: Option<&'static str>,
    pub jump_error: bool,
    jump_checked: Option<(String, String)>,
    pub seed_dirty: bool,
    seed_checked: Option<String>,
    pub submitted: Option<(i64, String)>,
    context: Option<(WorldId, Option<i64>, Option<String>)>,
    worlds: Vec<WorldChoice>,
    locale: u32,
}

fn default_version(context: Option<&WorldContext>) -> &'static str {
    context
        .and_then(|context| context.version.as_deref())
        .and_then(|version| {
            VERSIONS
                .iter()
                .copied()
                .find(|candidate| *candidate == version)
        })
        .unwrap_or("1.21.4")
}

/// One coordinate field: a finite number inside the world border.
pub(super) fn axis(text: &str) -> Option<f64> {
    let value = text.trim().parse::<f64>().ok()?;
    (value.is_finite() && value.abs() <= 29_900_000.).then_some(value)
}

impl MapView {
    pub(super) fn ensure_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_none() {
            let seed =
                cx.new(|cx| InputState::new(window, cx).placeholder(tr!("map-seed-placeholder")));
            let jump_x = cx.new(|cx| InputState::new(window, cx).placeholder("X"));
            let jump_z = cx.new(|cx| InputState::new(window, cx).placeholder("Z"));
            let version = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(
                        VERSIONS
                            .iter()
                            .map(|version| (*version).to_owned())
                            .collect::<Vec<_>>(),
                    ),
                    Some(IndexPath::new(VERSIONS.len() - 1)),
                    window,
                    cx,
                )
                .searchable(true)
            });
            let world = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(Vec::<WorldChoice>::new()),
                    None,
                    window,
                    cx,
                )
                .searchable(true)
            });
            let seed_focus = seed.read(cx).focus_handle(cx);
            cx.on_blur(&seed_focus, window, |this, _, cx| {
                if this.form.as_ref().is_some_and(|form| form.seed_dirty) {
                    this.apply_seed(cx);
                    cx.notify();
                }
            })
            .detach();
            cx.subscribe_in(&seed, window, |this, _, event: &InputEvent, _, cx| {
                match event {
                    InputEvent::Change => {
                        if let Some(form) = &mut this.form
                            && form.seed_checked.as_deref()
                                != Some(form.seed.read(cx).value().as_str())
                        {
                            form.seed_dirty = true;
                            form.seed_error = None;
                        }
                    }
                    InputEvent::PressEnter { .. } => this.apply_seed(cx),
                    _ => {}
                }
                cx.notify();
            })
            .detach();
            cx.subscribe_in(
                &version,
                window,
                |this, _, event: &SelectEvent<SearchableVec<String>>, _, cx| {
                    if let SelectEvent::Confirm(Some(_)) = event {
                        if let Some(form) = &mut this.form {
                            form.seed_dirty = true;
                        }
                        this.apply_seed(cx);
                        cx.notify();
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &world,
                window,
                |this, _, event: &SelectEvent<SearchableVec<WorldChoice>>, _, cx| {
                    if let SelectEvent::Confirm(Some(id)) = event
                        && let Some(world) = this
                            .contexts
                            .iter()
                            .find(|world| &world.context.world == id)
                    {
                        this.context = Some(world.context.clone());
                        if let Some(form) = &mut this.form {
                            form.seed_dirty = false;
                            form.seed_error = None;
                            form.seed_checked = None;
                            form.submitted = None;
                        }
                        this.reset_view();
                        this.context_changed();
                        this.refresh();
                        cx.notify();
                    }
                },
            )
            .detach();
            for input in [&jump_x, &jump_z] {
                cx.subscribe_in(input, window, |this, _, event: &InputEvent, _, cx| {
                    match event {
                        InputEvent::PressEnter { .. } => this.jump(cx),
                        InputEvent::Change => {
                            if let Some(form) = &mut this.form
                                && form.jump_checked.as_ref().is_none_or(|(x, z)| {
                                    x != form.jump_x.read(cx).value().as_str()
                                        || z != form.jump_z.read(cx).value().as_str()
                                })
                            {
                                form.jump_error = false;
                            }
                        }
                        _ => {}
                    }
                    cx.notify();
                })
                .detach();
            }
            self.form = Some(Form {
                seed,
                version,
                world,
                jump_x,
                jump_z,
                seed_error: None,
                jump_error: false,
                jump_checked: None,
                seed_dirty: false,
                seed_checked: None,
                submitted: None,
                context: None,
                worlds: vec![],
                locale: crate::i18n::generation(),
            });
        }
        let form = self.form.as_mut().unwrap();
        let stamp = self
            .context
            .as_ref()
            .map(|context| (context.world.clone(), context.seed, context.version.clone()));
        if stamp != form.context {
            if !form.seed_dirty {
                let value = self
                    .context
                    .as_ref()
                    .and_then(|context| context.seed)
                    .map(|seed| seed.to_string())
                    .unwrap_or_default();
                // Preserve a text seed after applying its Java hash.
                if lumilio_core::world_map::store::parse_seed(&form.seed.read(cx).value())
                    != self.context.as_ref().and_then(|context| context.seed)
                {
                    form.seed
                        .update(cx, |seed, cx| seed.set_value(value, window, cx));
                }
                let version = default_version(self.context.as_ref()).to_owned();
                form.version.update(cx, |select, cx| {
                    select.set_selected_value(&version, window, cx)
                });
            }
            form.context = stamp;
            let chosen = self.context.as_ref().map(|context| &context.world);
            form.world.update(cx, |select, cx| {
                if let Some(chosen) = chosen {
                    select.set_selected_value(chosen, window, cx);
                } else {
                    select.set_selected_index(None, window, cx);
                }
            });
        }
        let worlds: Vec<_> = self
            .contexts
            .iter()
            .filter(|world| {
                matches!(
                    world.context.world,
                    WorldId::Save { .. } | WorldId::Server { .. }
                )
            })
            .map(|world| WorldChoice {
                id: world.context.world.clone(),
                name: world.name.clone(),
            })
            .collect();
        if worlds != form.worlds {
            form.world.update(cx, |select, cx| {
                select.set_items(SearchableVec::new(worlds.clone()), window, cx);
                if let Some(context) = &self.context {
                    select.set_selected_value(&context.world, window, cx);
                }
            });
            form.worlds = worlds;
        }
        if form.locale != crate::i18n::generation() {
            form.seed.update(cx, |seed, cx| {
                seed.set_placeholder(tr!("map-seed-placeholder"), window, cx)
            });
            form.locale = crate::i18n::generation();
        }
    }

    /// Centres the map on the X and Z fields; an unreadable pair is flagged
    /// beside the fields and leaves the camera alone.
    pub(super) fn jump(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else {
            return;
        };
        let (x_text, z_text) = (
            form.jump_x.read(cx).value().to_string(),
            form.jump_z.read(cx).value().to_string(),
        );
        let (x, z) = (axis(&x_text), axis(&z_text));
        form.jump_checked = Some((x_text, z_text));
        form.jump_error = x.is_none() || z.is_none();
        if let (Some(x), Some(z)) = (x, z) {
            self.camera.x = x;
            self.camera.z = z;
            self.refresh();
        }
        cx.notify();
    }

    fn apply_seed(&mut self, cx: &mut Context<Self>) {
        let Some(form) = &mut self.form else {
            return;
        };
        form.seed_checked = Some(form.seed.read(cx).value().to_string());
        let Some(seed) = lumilio_core::world_map::store::parse_seed(&form.seed.read(cx).value())
        else {
            form.seed_error = Some("map-seed-empty");
            return;
        };
        let Some(version) = form.version.read(cx).selected_value().cloned() else {
            return;
        };
        if form.submitted.as_ref() == Some(&(seed, version.clone())) {
            form.seed_dirty = false;
            return;
        }
        let sent = self.connection.as_ref().is_some_and(|connection| {
            connection
                .send
                .try_send(Command::SaveSeed {
                    seed,
                    version: version.clone(),
                })
                .is_ok()
        });
        form.seed_error = (!sent).then_some("map-seed-save-failed");
        if sent {
            form.submitted = Some((seed, version));
            form.seed_dirty = false;
        }
    }
}
