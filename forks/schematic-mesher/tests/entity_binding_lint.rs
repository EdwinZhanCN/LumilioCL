//! Table lint: every layer and texture `data/entity_bindings.json` names is
//! one the game has, checked against the real baked bundle.
//!
//! Each row is tried on every 26.3 block it matches (and every entity its
//! layer patterns name). Its main layers must be in that version's
//! `entities.dat.z` and one of its textures in that version's `blocks.dat.z`;
//! rows the game draws as block models from 26.2 (`model_replaces`: beds and
//! signs) are checked against 26.1.2, their last entity version. Every other
//! layer it names (baby, overlay, armor alternatives) must exist in some
//! bundled version. The bundle is built by `cargo xtask bake-blocks` into
//! `target/block-assets/` (or named by `LUMILIO_BLOCK_BUNDLE` and
//! `LUMILIO_ENTITIES`); without it the lint says so and passes. LumilioCL
//! local change (see `forks/README.md`, plan schematic-studio T7).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use lumilio_block_assets::{Bundle, EntityBundle, EntityLayers};
use schematic_mesher::mesher::entity::bindings::{
    capture, expand, find_in, parse_table, Binding, BindingTable, Bound, LayerRef, OneOrMany,
};
use schematic_mesher::{InputBlock, ResourcePack};

const NEWEST: &str = "26.3";
/// The last version that draws beds and signs as entities.
const LAST_ENTITY_SIGNS_AND_BEDS: &str = "26.1.2";

struct Game {
    packs: HashMap<&'static str, ResourcePack>,
    layers: HashMap<&'static str, EntityLayers>,
    /// Layer ids of every bundled version.
    any_layer: BTreeSet<String>,
    /// Renderer classes some bundled version's looks name (T35).
    any_renderer: BTreeSet<String>,
    /// Every bundled version's looks, by entity id.
    looks: Vec<BTreeMap<String, lumilio_block_assets::EntityLook>>,
}

fn target_file(variable: &str, name: &str) -> PathBuf {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/block-assets")
                .join(name)
        })
}

fn game() -> Option<Game> {
    let blocks = target_file("LUMILIO_BLOCK_BUNDLE", "blocks.dat.z");
    let entities = target_file("LUMILIO_ENTITIES", "entities.dat.z");
    let (Ok(blocks), Ok(entities)) = (std::fs::read(&blocks), std::fs::read(&entities)) else {
        eprintln!(
            "skipped: no baked bundle at {} and {} (cargo xtask bake-blocks)",
            blocks.display(),
            entities.display()
        );
        return None;
    };
    let blocks = Bundle::read(&blocks).unwrap();
    let entities = EntityBundle::read(&entities).unwrap();
    let mut packs = HashMap::new();
    let mut layers = HashMap::new();
    for version in [NEWEST, LAST_ENTITY_SIGNS_AND_BEDS] {
        packs.insert(
            version,
            ResourcePack::from_baked(&blocks.pack(version).unwrap()),
        );
        layers.insert(version, entities.layers_of(version));
    }
    let any_layer = entities
        .versions
        .iter()
        .flat_map(|version| version.layers.keys().cloned())
        .collect();
    let any_renderer = entities
        .versions
        .iter()
        .flat_map(|version| version.looks.values())
        .flat_map(|look| look.renderer.iter().cloned())
        .collect();
    let looks = entities
        .versions
        .iter()
        .map(|version| version.looks.clone())
        .collect();
    Some(Game {
        packs,
        layers,
        any_layer,
        any_renderer,
        looks,
    })
}

/// The blocks and entities `binding` (row `index` of `table`) is tried on.
fn instances(
    table: &'static BindingTable,
    index: usize,
    blocks: &[String],
    layers: &EntityLayers,
) -> Vec<Bound> {
    let binding = &table.bindings[index];
    let mut names = BTreeSet::new();
    for pattern in &binding.patterns {
        let (kind, pattern) = pattern.split_once(':').unwrap();
        if kind == "block" {
            names.extend(
                blocks
                    .iter()
                    .filter(|id| capture(pattern, id).is_some())
                    .map(|id| format!("minecraft:{id}")),
            );
            continue;
        }
        // An entity's captures come from the layers its row names.
        let mut values: Vec<HashMap<String, String>> = Vec::new();
        for layer in &binding.layers {
            for id in layers.keys() {
                if let Some(found) = capture(layer.layer(), id) {
                    values.push(found.into_iter().collect());
                }
            }
        }
        if !pattern.contains('{') {
            values = vec![HashMap::new()];
        }
        for vars in values {
            if let Some(id) = expand(pattern, |name| vars.get(name).cloned()) {
                names.insert(format!("entity:{id}"));
            }
        }
    }
    let mut out = Vec::new();
    for name in names {
        for properties in property_sets(&binding.when) {
            let mut block = InputBlock::new(name.clone());
            block.properties = properties;
            // Only where an earlier row does not take the block first.
            if let Some(bound) = find_in(table, &block) {
                if std::ptr::eq(bound.binding, binding) {
                    out.push(bound);
                }
            }
        }
    }
    out
}

/// Every combination of the allowed values of `when`.
fn property_sets(when: &BTreeMap<String, OneOrMany>) -> Vec<HashMap<String, String>> {
    let mut sets = vec![HashMap::new()];
    for (name, allowed) in when {
        sets = sets
            .into_iter()
            .flat_map(|set| {
                allowed.items().iter().map(move |value| {
                    let mut set = set.clone();
                    set.insert(name.clone(), value.clone());
                    set
                })
            })
            .collect();
    }
    sets
}

/// Problems with `table`, as readable lines.
fn lint(table: &'static BindingTable, game: &Game) -> Vec<String> {
    let mut problems = BTreeSet::new();
    for (index, binding) in table.bindings.iter().enumerate() {
        let version = if binding.model_replaces {
            LAST_ENTITY_SIGNS_AND_BEDS
        } else {
            NEWEST
        };
        let pack = &game.packs[version];
        let layers = &game.layers[version];
        let blocks: Vec<String> = pack
            .blockstates
            .get("minecraft")
            .map(|states| states.keys().cloned().collect())
            .unwrap_or_default();
        let row = format!("row {index} {:?}", binding.patterns);
        let bound = instances(table, index, &blocks, layers);
        if bound.is_empty() {
            problems.insert(format!("{row}: matches no {version} block or entity"));
        }
        for bound in &bound {
            check_row(
                binding,
                bound,
                version,
                pack,
                layers,
                game,
                &row,
                &mut problems,
            );
        }
    }
    lint_renderers(table, game, &mut problems);
    lint_redundant(table, game, &mut problems);
    problems.into_iter().collect()
}

/// A row that only names an entity's main layer and texture, placed as a
/// living entity, when every version's look gives that same layer and
/// texture: the row made from the look draws it alike, so the hand-written
/// one is a patch to remove (T35, D32).
fn lint_redundant(table: &'static BindingTable, game: &Game, problems: &mut BTreeSet<String>) {
    for binding in &table.bindings {
        let [pattern] = binding.patterns.as_slice() else {
            continue;
        };
        let Some(id) = pattern
            .strip_prefix("entity:")
            .filter(|id| !id.contains('{'))
        else {
            continue;
        };
        let plain = binding.when.is_empty()
            && binding.vars.is_empty()
            && binding.defaults.is_empty()
            && binding.compose.is_none()
            && binding.overlays.is_empty()
            && binding.armor.is_empty()
            && binding.baby.is_none()
            && binding.pose.is_empty()
            && binding.pose_when.is_empty()
            && binding.pose_properties.is_empty()
            && binding.rider_offset.is_none()
            && binding.renderer_scale.is_none()
            && !binding.humanoid
            && binding.aim.is_none()
            && binding.carries.is_none()
            && binding.tint.is_none()
            && binding.facing == schematic_mesher::mesher::entity::bindings::Facing::Horizontal
            && matches!(&binding.transform, Some(schematic_mesher::mesher::entity::bindings::TransformRef::Named(name)) if name == "mob");
        let (Some(LayerRef::Id(layer)), Some(OneOrMany::One(texture)), 1) = (
            binding.layers.first(),
            binding.texture.as_ref(),
            binding.layers.len(),
        ) else {
            continue;
        };
        let key = format!("minecraft:{id}");
        let texture = format!("minecraft:textures/{texture}.png");
        let alike = game.looks.iter().all(|looks| {
            looks.get(&key).is_some_and(|look| {
                look.is_living()
                    && look.layers.first() == Some(layer)
                    && look.texture.as_deref() == Some(texture.as_str())
            })
        });
        if plain && alike {
            problems.insert(format!(
                "row {pattern}: every version's look draws it alike"
            ));
        }
    }
}

/// Rows by renderer class (T35): the class is one some version's renderer
/// has; every layer the row names, its variables at their defaults, is in
/// some version; and every texture so named, but the look's own, is in the
/// newest pack.
fn lint_renderers(table: &'static BindingTable, game: &Game, problems: &mut BTreeSet<String>) {
    for (class, rows) in &table.renderers {
        if !game.any_renderer.contains(class) {
            problems.insert(format!("renderer {class}: no version has it"));
        }
        for (index, binding) in rows.iter().enumerate() {
            let at = format!("renderer {class} row {index}");
            let var = |name: &str| {
                binding
                    .vars
                    .get(name)
                    .or_else(|| binding.defaults.get(name))
                    .cloned()
            };
            let mut layers: Vec<&LayerRef> = binding.layers.iter().collect();
            layers.extend(binding.overlays.iter().flat_map(|overlay| &overlay.layers));
            if let Some(baby) = &binding.baby {
                layers.extend(&baby.layers);
            }
            for reference in layers {
                if let Some(id) = expand(reference.layer(), var) {
                    if !game.any_layer.contains(&id) {
                        problems.insert(format!("{at}: no version has layer {id}"));
                    }
                }
            }
            let mut textures: Vec<&OneOrMany> = binding.texture.iter().collect();
            textures.extend(binding.overlays.iter().map(|overlay| &overlay.texture));
            textures.extend(binding.baby.iter().filter_map(|baby| baby.texture.as_ref()));
            let pack = &game.packs[NEWEST];
            for candidates in textures {
                let expanded: Vec<String> = candidates
                    .items()
                    .iter()
                    .filter(|pattern| !pattern.contains("{look_texture}"))
                    .filter_map(|pattern| expand(pattern, var))
                    .collect();
                if !expanded.is_empty()
                    && !expanded.iter().any(|key| pack.get_texture(key).is_some())
                {
                    problems.insert(format!("{at}: texture {expanded:?} not in {NEWEST}"));
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_row(
    binding: &Binding,
    bound: &Bound,
    version: &str,
    pack: &ResourcePack,
    layers: &EntityLayers,
    game: &Game,
    row: &str,
    problems: &mut BTreeSet<String>,
) {
    let name = bound
        .properties
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");
    let at = format!("{row} [{name}]");
    let layer_id = |reference: &LayerRef| bound.expand(reference.layer());
    for reference in &binding.layers {
        match layer_id(reference) {
            Some(id) if layers.contains_key(&id) => {}
            Some(id) => {
                problems.insert(format!("{at}: no layer {id} in {version}"));
            }
            None => {
                problems.insert(format!("{at}: unset variable in {}", reference.layer()));
            }
        }
    }
    let mut elsewhere: Vec<&LayerRef> = Vec::new();
    if let Some(baby) = &binding.baby {
        elsewhere.extend(&baby.layers);
    }
    for overlay in &binding.overlays {
        elsewhere.extend(&overlay.layers);
    }
    for candidates in binding.armor.values() {
        elsewhere.extend(candidates);
    }
    for reference in elsewhere {
        let id = layer_id(reference).unwrap_or_else(|| reference.layer().to_owned());
        if !game.any_layer.contains(&id) {
            problems.insert(format!("{at}: no version has layer {id}"));
        }
    }

    let mut textures: Vec<(&str, &OneOrMany, BTreeMap<String, OneOrMany>)> = Vec::new();
    if let Some(texture) = &binding.texture {
        textures.push(("texture", texture, BTreeMap::new()));
    }
    for reference in &binding.layers {
        if let Some(texture) = reference.texture() {
            textures.push(("layer texture", texture, BTreeMap::new()));
        }
        for texture in reference
            .bone_textures()
            .into_iter()
            .flatten()
            .map(|(_, t)| t)
        {
            textures.push(("bone texture", texture, BTreeMap::new()));
        }
    }
    if let Some(texture) = binding.baby.as_ref().and_then(|baby| baby.texture.as_ref()) {
        textures.push(("baby texture", texture, BTreeMap::new()));
    }
    for overlay in &binding.overlays {
        textures.push(("overlay texture", &overlay.texture, overlay.when.clone()));
    }
    for (what, candidates, when) in textures {
        for extra in property_sets(&when) {
            let var = |name: &str| {
                extra
                    .get(name)
                    .map(String::as_str)
                    .or_else(|| bound.var(name))
                    .map(str::to_owned)
            };
            let expanded: Vec<String> = candidates
                .items()
                .iter()
                .filter_map(|pattern| expand(pattern, var))
                .collect();
            if !expanded.iter().any(|key| pack.get_texture(key).is_some()) {
                problems.insert(format!("{at}: {what} {expanded:?} not in {version}"));
            }
        }
    }
}

#[test]
fn every_binding_names_real_layers_and_textures() {
    let Some(game) = game() else { return };
    let table = schematic_mesher::mesher::entity::bindings::table();
    let problems = lint(table, &game);
    assert!(problems.is_empty(), "{problems:#?}");
}

/// The lint catches a misspelt layer, a misspelt texture and a row no block
/// reaches.
#[test]
fn the_lint_reports_what_the_game_lacks() {
    let Some(game) = game() else { return };
    let text = include_str!("../data/entity_bindings.json")
        .replacen("minecraft:bell#main", "minecraft:bells#main", 1)
        .replacen("entity/creeper/creeper\"", "entity/creeper/creepr\"", 1)
        .replacen("\"block:bell\"", "\"block:bellz\"", 1);
    let table: &'static BindingTable = Box::leak(Box::new(parse_table(&text).unwrap()));
    let problems = lint(table, &game).join("\n");
    assert!(problems.contains("matches no 26.3 block"), "{problems}");
    assert!(problems.contains("entity/creeper/creepr"), "{problems}");
    let text = include_str!("../data/entity_bindings.json").replacen(
        "minecraft:bell#main",
        "minecraft:bells#main",
        1,
    );
    let table: &'static BindingTable = Box::leak(Box::new(parse_table(&text).unwrap()));
    let problems = lint(table, &game).join("\n");
    assert!(
        problems.contains("no layer minecraft:bells#main"),
        "{problems}"
    );
    // A row a look draws alike in every version is reported.
    let text = include_str!("../data/entity_bindings.json").replacen(
        "  \"bindings\": [",
        "  \"bindings\": [\n    { \"match\": [\"entity:creeper\"], \"layers\": [\"minecraft:creeper#main\"], \"texture\": \"entity/creeper/creeper\", \"transform\": \"mob\", \"facing\": \"horizontal\" },",
        1,
    );
    let table: &'static BindingTable = Box::leak(Box::new(parse_table(&text).unwrap()));
    let problems = lint(table, &game).join("\n");
    assert!(
        problems.contains("row entity:creeper: every version's look draws it alike"),
        "{problems}"
    );
    // A renderer row names a class, layer or texture no version has.
    let text = include_str!("../data/entity_bindings.json")
        .replacen("\"LeashKnotRenderer\"", "\"LeashKnotRendrer\"", 1)
        .replacen(
            "minecraft:wind_charge#main",
            "minecraft:wind_charges#main",
            1,
        )
        .replacen(
            "entity/enderdragon/dragon_eyes",
            "entity/enderdragon/dragon_eye",
            1,
        );
    let table: &'static BindingTable = Box::leak(Box::new(parse_table(&text).unwrap()));
    let problems = lint(table, &game).join("\n");
    assert!(
        problems.contains("renderer LeashKnotRendrer: no version has it"),
        "{problems}"
    );
    assert!(
        problems.contains("no version has layer minecraft:wind_charges#main"),
        "{problems}"
    );
    assert!(
        problems.contains("entity/enderdragon/dragon_eye\""),
        "{problems}"
    );
}
