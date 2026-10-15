use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;
use std::path::{Path, PathBuf};

use super::*;

const FIXTURE: &[u8] = include_bytes!("fixture_26_3.json");

fn fixture() -> EntityDump {
    EntityDump::from_json(FIXTURE).unwrap()
}

#[test]
fn a_dump_reads_with_every_field() {
    let dump = fixture();
    assert_eq!(dump.layers.len(), 4);
    let chest = &dump.layers["minecraft:chest#main"];
    assert_eq!(chest.texture_size, [64, 64]);
    let lid = &chest.root.children["lid"];
    assert_eq!(lid.pivot, [0.0, 9.0, 1.0]);
    assert_eq!(lid.scale, [1.0; 3]);
    assert_eq!(lid.cubes[0].size, [14.0, 5.0, 14.0]);
    assert_eq!(lid.cubes[0].faces, None);

    let left = &dump.layers["minecraft:double_chest_left#main"]
        .root
        .children["bottom"];
    assert_eq!(
        left.cubes[0].faces.as_deref(),
        Some(
            &[
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::East
            ][..]
        )
    );

    let elytra = &dump.layers["minecraft:elytra_baby#main"].root;
    assert_eq!(elytra.scale, [0.5; 3]);
    let wing = &elytra.children["right_wing"];
    assert!(wing.cubes[0].mirror);
    assert_eq!(wing.cubes[0].inflate, [1.0; 3]);
    assert_eq!(wing.rotation, [0.2617994, 0.0, 0.2617994]);

    let arrow = &dump.layers["minecraft:arrow#main"].root.children["cross_1"];
    assert_eq!(arrow.cubes[0].uv_scale, [1.0, 0.8]);
}

#[test]
fn a_dump_of_another_format_is_refused() {
    let text =
        String::from_utf8(FIXTURE.to_vec())
            .unwrap()
            .replacen("\"format\":1", "\"format\":9", 1);
    assert_eq!(
        EntityDump::from_json(text.as_bytes()),
        Err(ContainerError::UnsupportedFormat(9))
    );
}

#[test]
fn versions_share_layers_and_read_back_equal() {
    let dump = fixture();
    let mut newer = dump.clone();
    newer
        .layers
        .get_mut("minecraft:chest#main")
        .unwrap()
        .root
        .children
        .get_mut("lid")
        .unwrap()
        .pivot[1] = 10.0;
    // A borrowed version must name a dumped one.
    assert_eq!(
        write_entities([("1.16.5", EntitySource::SameAs("1.17", BTreeMap::new()))]).err(),
        Some(ContainerError::MissingPack("1.17".to_owned()))
    );
    let ordered = || {
        [
            ("1.16.5", EntitySource::SameAs("1.17", BTreeMap::new())),
            ("1.17", EntitySource::Dumped(&dump)),
            ("26.3", EntitySource::Dumped(&newer)),
        ]
    };
    let bytes = write_entities(ordered()).unwrap();
    assert_eq!(bytes, write_entities(ordered()).unwrap());

    let bundle = EntityBundle::read(&bytes).unwrap();
    // Four layers, one of which changed: five stored.
    assert_eq!(bundle.layers.len(), 5);
    assert_eq!(
        bundle.version("1.16.5").unwrap().geometry_from.as_deref(),
        Some("1.17")
    );
    let order: Vec<&str> = bundle.versions.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(order, ["1.16.5", "1.17", "26.3"]);
    for (name, layer) in &dump.layers {
        assert_eq!(bundle.layer("1.17", name), Some(layer));
        assert_eq!(bundle.layer("1.16.5", name), Some(layer));
        assert_eq!(bundle.layer("26.3", name), newer.layers.get(name));
    }
    assert_eq!(bundle.layer("1.12.2", "minecraft:chest#main"), None);
    assert_eq!(*bundle.layers_of("1.16.5"), dump.layers);
    assert_eq!(*bundle.layers_of("26.3"), newer.layers);
    assert!(bundle.layers_of("1.12.2").is_empty());
}

/// T34: a format 2 dump's renderer looks reach the entity file and come back
/// with the version's layers; a version borrowing geometry has none, and a
/// format 1 dump (layers only) still reads.
#[test]
fn renderer_looks_travel_with_their_version() {
    let mut dump = fixture();
    dump.format = DUMP_FORMAT;
    let bee = EntityLook {
        renderer: vec!["BeeRenderer".into(), "LivingEntityRenderer".into()],
        entity: vec!["Bee".into(), "LivingEntity".into(), "Entity".into()],
        layers: vec![
            "minecraft:bee#main".into(),
            "minecraft:bee_baby#main".into(),
        ],
        texture: Some("minecraft:textures/entity/bee/bee.png".into()),
        baby_texture: Some("minecraft:textures/entity/bee/bee_baby.png".into()),
        ..EntityLook::default()
    };
    dump.entities.insert("minecraft:bee".into(), bee.clone());
    let text = dump.to_json().unwrap();
    assert_eq!(EntityDump::from_json(&text).unwrap(), dump);
    assert!(bee.is_living());

    let bytes = write_entities([
        ("1.16.5", EntitySource::SameAs("26.3", BTreeMap::new())),
        ("26.3", EntitySource::Dumped(&dump)),
    ])
    .unwrap();
    let bundle = EntityBundle::read(&bytes).unwrap();
    assert_eq!(
        bundle.layers_of("26.3").looks.get("minecraft:bee"),
        Some(&bee)
    );
    assert!(bundle.layers_of("1.16.5").looks.is_empty());

    let older = String::from_utf8(FIXTURE.to_vec()).unwrap();
    assert!(older.contains("\"format\":1"));
    assert!(
        EntityDump::from_json(older.as_bytes())
            .unwrap()
            .entities
            .is_empty()
    );
}

#[test]
fn a_damaged_entity_file_is_a_readable_error() {
    let dump = fixture();
    let bytes = write_entities([("26.3", EntitySource::Dumped(&dump))]).unwrap();
    assert_eq!(
        EntityBundle::read(b"not zstd").err(),
        Some(ContainerError::NotABundle)
    );
    assert!(EntityBundle::read(&bytes[..bytes.len() / 2]).is_err());
    let raw = zstd::decode_all(bytes.as_slice()).unwrap();
    let mut newer = raw.clone();
    newer[8] = 2;
    let newer = zstd::encode_all(newer.as_slice(), 1).unwrap();
    assert_eq!(
        EntityBundle::read(&newer).err(),
        Some(ContainerError::UnsupportedFormat(2))
    );
    let text = String::from_utf8(raw[12..].to_vec())
        .unwrap()
        .replacen(":0", ":99", 1);
    let mut past = raw[..12].to_vec();
    past.extend_from_slice(text.as_bytes());
    let past = zstd::encode_all(past.as_slice(), 1).unwrap();
    assert!(matches!(
        EntityBundle::read(&past),
        Err(ContainerError::Corrupt(_))
    ));
}

// ── Regression bridge (T6) ─────────────────────────────────────────────────
//
// The numbers below are the hand-copied geometry in
// `forks/schematic-mesher/src/mesher/entity/{chest,bed,banner,shulker,sign}.rs`
// (MIT, schematic-mesher). They are compared box by box, in world space, with
// the baked dump of the newest version that has the layer: renderer
// transforms the hand copy folds into its parts are applied to the dump as a
// wrapper bone, so only geometry from the game is compared.

fn bone(pivot: [f32; 3], rotation: [f32; 3], cubes: Vec<Cube>) -> Bone {
    Bone {
        pivot,
        rotation,
        scale: [1.0; 3],
        cubes,
        children: BTreeMap::new(),
    }
}

fn cube(origin: [f32; 3], size: [f32; 3], uv: [f32; 2]) -> Cube {
    Cube {
        origin,
        size,
        uv,
        uv_scale: [1.0; 2],
        inflate: [0.0; 3],
        mirror: false,
        faces: None,
    }
}

fn root(children: Vec<Bone>) -> Bone {
    let mut root = bone([0.0; 3], [0.0; 3], vec![]);
    for (at, child) in children.into_iter().enumerate() {
        root.children.insert(at.to_string(), child);
    }
    root
}

/// The hand copy's renderer wrapper around the game's parts.
fn wrapped(pivot: [f32; 3], rotation: [f32; 3], scale: f32, inner: Bone) -> Bone {
    let mut wrapper = bone(pivot, rotation, vec![]);
    wrapper.scale = [scale; 3];
    wrapper.children.insert("game".to_owned(), inner);
    wrapper
}

type Matrix = [[f64; 4]; 4];

fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 4];
    for (row, line) in out.iter_mut().enumerate() {
        for (column, cell) in line.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[row][k] * b[k][column]).sum();
        }
    }
    out
}

/// Translate to the pivot, rotate Z, Y, X, scale: the game's `ModelPart`.
fn local(bone: &Bone) -> Matrix {
    let [x, y, z] = bone.pivot.map(f64::from);
    let mut m: Matrix = [
        [1.0, 0.0, 0.0, x],
        [0.0, 1.0, 0.0, y],
        [0.0, 0.0, 1.0, z],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let [rx, ry, rz] = bone.rotation.map(f64::from);
    let (s, c) = rz.sin_cos();
    m = multiply(
        &m,
        &[
            [c, -s, 0.0, 0.0],
            [s, c, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    );
    let (s, c) = ry.sin_cos();
    m = multiply(
        &m,
        &[
            [c, 0.0, s, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [-s, 0.0, c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    );
    let (s, c) = rx.sin_cos();
    m = multiply(
        &m,
        &[
            [1.0, 0.0, 0.0, 0.0],
            [0.0, c, -s, 0.0],
            [0.0, s, c, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    );
    let [sx, sy, sz] = bone.scale.map(f64::from);
    multiply(
        &m,
        &[
            [sx, 0.0, 0.0, 0.0],
            [0.0, sy, 0.0, 0.0],
            [0.0, 0.0, sz, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    )
}

/// A box in world space: its corners (rounded to 1/1000), texture offset and size.
type WorldBox = (Vec<[i64; 3]>, [i64; 2], [i64; 3]);

fn boxes(bone: &Bone, parent: &Matrix, out: &mut Vec<WorldBox>) {
    let m = multiply(parent, &local(bone));
    let round = |value: f64| (value * 1000.0).round() as i64;
    for cube in &bone.cubes {
        let low: [f64; 3] =
            std::array::from_fn(|i| f64::from(cube.origin[i]) - f64::from(cube.inflate[i]));
        let high: [f64; 3] = std::array::from_fn(|i| {
            f64::from(cube.origin[i] + cube.size[i]) + f64::from(cube.inflate[i])
        });
        let mut corners = Vec::new();
        for corner in 0..8 {
            let p = [
                if corner & 1 == 0 { low[0] } else { high[0] },
                if corner & 2 == 0 { low[1] } else { high[1] },
                if corner & 4 == 0 { low[2] } else { high[2] },
            ];
            corners.push(std::array::from_fn(|row| {
                round((0..3).map(|k| m[row][k] * p[k]).sum::<f64>() + m[row][3])
            }));
        }
        corners.sort_unstable();
        out.push((
            corners,
            cube.uv.map(|v| round(f64::from(v))),
            cube.size.map(|v| round(f64::from(v))),
        ));
    }
    for child in bone.children.values() {
        boxes(child, &m, out);
    }
}

fn world(bone: &Bone) -> Vec<WorldBox> {
    let identity = std::array::from_fn(|row| std::array::from_fn(|col| f64::from(row == col)));
    let mut out = Vec::new();
    boxes(bone, &identity, &mut out);
    out.sort();
    out
}

fn entities_file() -> PathBuf {
    if let Some(file) = std::env::var_os("LUMILIO_ENTITIES") {
        return file.into();
    }
    let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"),
        PathBuf::from,
    );
    target.join("block-assets/entities.dat.z")
}

/// The newest version whose dump has `layer`, and that layer.
fn newest<'a>(bundle: &'a EntityBundle, layer: &str) -> (&'a str, &'a EntityLayer) {
    bundle
        .versions
        .iter()
        .rev()
        .find_map(|version| {
            let at = *version.layers.get(layer)?;
            Some((version.id.as_str(), &bundle.layers[at as usize]))
        })
        .unwrap_or_else(|| panic!("no baked version has {layer}"))
}

/// Several layers drawn together (a banner's pole and flag) as one tree.
fn joined(layers: &[&EntityLayer]) -> Bone {
    root(layers.iter().map(|layer| layer.root.clone()).collect())
}

#[test]
fn baked_geometry_matches_the_hand_copied_models() {
    let file = entities_file();
    let Ok(bytes) = std::fs::read(&file) else {
        eprintln!(
            "skipped: {} is missing; run `cargo xtask bake-blocks` to check the regression bridge",
            file.display()
        );
        return;
    };
    let bundle = EntityBundle::read(&bytes).unwrap();
    let latest = bundle.versions.last().unwrap().id.clone();
    let mut checked = Vec::new();
    let mut check = |name: &str, layers: &[&str], wrap: &dyn Fn(Bone) -> Bone, hand: Bone| {
        let found: Vec<_> = layers.iter().map(|layer| newest(&bundle, layer)).collect();
        let version = found[0].0;
        let game = wrap(joined(&found.iter().map(|(_, l)| *l).collect::<Vec<_>>()));
        assert_eq!(world(&game), world(&hand), "{name} in {version}");
        checked.push(format!("{name}@{version}"));
        version.to_owned()
    };
    let same = |bone: Bone| bone;

    // chest.rs: chest_model and double_chest_model.
    let chest = root(vec![
        bone(
            [0.0; 3],
            [0.0; 3],
            vec![cube([1.0, 0.0, 1.0], [14.0, 10.0, 14.0], [0.0, 19.0])],
        ),
        bone(
            [0.0, 9.0, 1.0],
            [0.0; 3],
            vec![cube([1.0, 0.0, 0.0], [14.0, 5.0, 14.0], [0.0, 0.0])],
        ),
        bone(
            [0.0, 8.0, 0.0],
            [0.0; 3],
            vec![cube([7.0, -1.0, 15.0], [2.0, 4.0, 1.0], [0.0, 0.0])],
        ),
    ]);
    assert_eq!(
        check("chest", &["minecraft:chest#main"], &same, chest),
        latest
    );
    for (side, bottom, lock) in [
        ("left", [0.0, 0.0, 1.0], [0.0, -1.0, 15.0]),
        ("right", [1.0, 0.0, 1.0], [15.0, -1.0, 15.0]),
    ] {
        let double = root(vec![
            bone(
                [0.0; 3],
                [0.0; 3],
                vec![cube(bottom, [15.0, 10.0, 14.0], [0.0, 19.0])],
            ),
            bone(
                [0.0, 9.0, 1.0],
                [0.0; 3],
                vec![cube([bottom[0], 0.0, 0.0], [15.0, 5.0, 14.0], [0.0, 0.0])],
            ),
            bone(
                [0.0, 8.0, 0.0],
                [0.0; 3],
                vec![cube(lock, [1.0, 4.0, 1.0], [0.0, 0.0])],
            ),
        ]);
        let layer = format!("minecraft:double_chest_{side}#main");
        assert_eq!(check(&layer, &[layer.as_str()], &same, double), latest);
    }

    // shulker.rs: base and lid inside the renderer's Y flip.
    let shulker = root(vec![
        bone(
            [0.0, 24.0, 0.0],
            [0.0; 3],
            vec![cube([-8.0, -8.0, -8.0], [16.0, 8.0, 16.0], [0.0, 28.0])],
        ),
        bone(
            [0.0, 24.0, 0.0],
            [0.0; 3],
            vec![cube([-8.0, -16.0, -8.0], [16.0, 12.0, 16.0], [0.0, 0.0])],
        ),
    ]);
    assert_eq!(
        check("shulker", &["minecraft:shulker_box#main"], &same, shulker),
        latest
    );

    // banner.rs: pole (standing only), bar and flag; the renderer's root
    // (position, X flip and 2/3 scale) is a separate bone there.
    let standing = root(vec![
        bone(
            [0.0; 3],
            [0.0; 3],
            vec![cube([-1.0, -42.0, -1.0], [2.0, 42.0, 2.0], [44.0, 0.0])],
        ),
        bone(
            [0.0; 3],
            [0.0; 3],
            vec![cube([-10.0, -44.0, -1.0], [20.0, 2.0, 2.0], [0.0, 42.0])],
        ),
        bone(
            [0.0, -44.0, 0.0],
            [0.0; 3],
            vec![cube([-10.0, 0.0, -2.0], [20.0, 40.0, 1.0], [0.0, 0.0])],
        ),
    ]);
    assert_eq!(
        check(
            "standing banner",
            &[
                "minecraft:standing_banner#main",
                "minecraft:standing_banner#flag"
            ],
            &same,
            standing,
        ),
        latest
    );
    let wall = root(vec![
        bone(
            [0.0; 3],
            [0.0; 3],
            vec![cube([-10.0, -20.5, 9.5], [20.0, 2.0, 2.0], [0.0, 42.0])],
        ),
        bone(
            [0.0, -20.5, 10.5],
            [0.0; 3],
            vec![cube([-10.0, 0.0, -2.0], [20.0, 40.0, 1.0], [0.0, 0.0])],
        ),
    ]);
    assert_eq!(
        check(
            "wall banner",
            &["minecraft:wall_banner#main", "minecraft:wall_banner#flag"],
            &same,
            wall,
        ),
        latest
    );

    // bed.rs: the mattress carries the renderer's lift and quarter turn; the
    // legs are written as unrotated boxes where the game's rotated legs land.
    let mut beds = Vec::new();
    let bed_wrap = |inner: Bone| wrapped([0.0, 9.0, 0.0], [FRAC_PI_2, 0.0, 0.0], 1.0, inner);
    for (part, main_uv, left_uv, right_uv, leg_z) in [
        ("head", [0.0, 0.0], [50.0, 6.0], [50.0, 18.0], 0.0),
        ("foot", [0.0, 22.0], [50.0, 0.0], [50.0, 12.0], 13.0),
    ] {
        let hand = root(vec![
            bone(
                [0.0, 9.0, 0.0],
                [FRAC_PI_2, 0.0, 0.0],
                vec![cube([0.0; 3], [16.0, 16.0, 6.0], main_uv)],
            ),
            bone(
                [0.0; 3],
                [0.0; 3],
                vec![cube([0.0, 0.0, leg_z], [3.0; 3], left_uv)],
            ),
            bone(
                [0.0; 3],
                [0.0; 3],
                vec![cube([13.0, 0.0, leg_z], [3.0; 3], right_uv)],
            ),
        ]);
        let layer = format!("minecraft:bed_{part}#main");
        beds.push(check(
            &format!("bed {part}"),
            &[layer.as_str()],
            &bed_wrap,
            hand,
        ));
    }
    let (bed_head, bed_foot) = (beds[0].clone(), beds[1].clone());

    // sign.rs: board and stick; every part there carries the same renderer
    // placement (position, X flip, 2/3 scale), so the boxes are compared
    // without it.
    let board = || {
        bone(
            [0.0; 3],
            [0.0; 3],
            vec![cube([-12.0, -14.0, -1.0], [24.0, 12.0, 2.0], [0.0, 0.0])],
        )
    };
    let stick = bone(
        [0.0; 3],
        [0.0; 3],
        vec![cube([-1.0, -2.0, -1.0], [2.0, 14.0, 2.0], [0.0, 14.0])],
    );
    let standing = check(
        "standing sign",
        &["minecraft:sign/standing/oak#main"],
        &same,
        root(vec![board(), stick]),
    );
    let wall = check(
        "wall sign",
        &["minecraft:sign/wall/oak#main"],
        &same,
        root(vec![board()]),
    );
    // 26.2 made beds and signs block models; their last entity layers are 26.1.2's.
    for version in [bed_head, bed_foot, standing, wall] {
        assert_eq!(version, LAST_ENTITY_SIGNS_AND_BEDS);
    }
    eprintln!("bridge checked: {}", checked.join(", "));
}

const LAST_ENTITY_SIGNS_AND_BEDS: &str = "26.1.2";

#[test]
fn the_canonical_json_is_a_fixed_point() {
    let canonical = fixture().to_json().unwrap();
    assert_ne!(
        canonical, FIXTURE,
        "the dumper's own spelling is not canonical"
    );
    let again = EntityDump::from_json(&canonical)
        .unwrap()
        .to_json()
        .unwrap();
    assert_eq!(again, canonical);
    assert_eq!(EntityDump::from_json(&canonical).unwrap(), fixture());
}

#[test]
fn the_dumper_is_installed_once_and_run_with_its_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let folder = install_dumper(dir.path()).unwrap();
    let class = folder.join(DUMPER_CLASS_FILE);
    assert_eq!(std::fs::read(&class).unwrap(), DUMPER_CLASS);
    assert_eq!(&DUMPER_CLASS[..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
    // Class file major 65 is Java 21.
    assert_eq!(u16::from_be_bytes([DUMPER_CLASS[6], DUMPER_CLASS[7]]), 65);
    std::fs::write(&class, b"stale").unwrap();
    install_dumper(dir.path()).unwrap();
    assert_eq!(std::fs::read(&class).unwrap(), DUMPER_CLASS);

    let jar = PathBuf::from("client.jar");
    let arguments = dumper_arguments(&folder, &[jar], Path::new("out.json"), None);
    let separator = if cfg!(windows) { ";" } else { ":" };
    assert_eq!(
        arguments[2],
        std::ffi::OsString::from(format!("{}{separator}client.jar", folder.display()))
    );
    assert_eq!(arguments[3], DUMPER_MAIN);
    assert_eq!(arguments.last().unwrap(), "out.json");
    assert_eq!(
        dumper_failure("WARNING: x\nException in thread \"main\" boom\n\tat a.b\nCaused by: c"),
        "Exception in thread \"main\" boom | Caused by: c"
    );
}

/// T34: renderers before 1.21.2 cannot say their textures; a newer version
/// lends its texture for the same entity drawing the same main layer, where
/// the older version's pack has it. Versions borrowing geometry borrow the
/// looks their packs can draw.
#[test]
fn older_versions_borrow_textures_their_packs_have() {
    let look = |layer: &str, texture: Option<&str>, baby: Option<&str>| EntityLook {
        renderer: vec!["LivingEntityRenderer".into()],
        layers: vec![layer.into()],
        texture: texture.map(Into::into),
        baby_texture: baby.map(Into::into),
        ..EntityLook::default()
    };
    let dump = |looks: Vec<(&str, EntityLook)>| EntityDump {
        format: DUMP_FORMAT,
        layers: BTreeMap::new(),
        entities: looks
            .into_iter()
            .map(|(id, look)| (id.to_owned(), look))
            .collect(),
        entities_error: None,
    };
    let bee = "bee/bee.png";
    let unbuilt = EntityLook {
        error: Some("NullPointerException building the renderer".into()),
        ..EntityLook::default()
    };
    let mut older = dump(vec![
        ("minecraft:bee", look("minecraft:bee#main", None, None)),
        ("minecraft:cow", look("minecraft:cow#main", None, None)),
        ("minecraft:pig", look("minecraft:pig#old", None, None)),
        ("minecraft:zombified_piglin", unbuilt),
    ]);
    // Its own layers: the piglin's adult model, not the baby's.
    older.layers.insert(
        "minecraft:zombified_piglin#main".into(),
        EntityLayer {
            texture_size: [64, 64],
            root: root(Vec::new()),
        },
    );
    let mut piglin = look(
        "minecraft:zombified_piglin#main",
        Some("piglin/zombified_piglin.png"),
        None,
    );
    piglin
        .layers
        .push("minecraft:zombified_piglin_baby#main".into());
    let mut dumps = vec![
        ("1.20.1".to_owned(), older),
        (
            "1.21.4".to_owned(),
            dump(vec![(
                "minecraft:bee",
                look("minecraft:bee#main", None, None),
            )]),
        ),
        (
            "26.3".to_owned(),
            dump(vec![
                (
                    "minecraft:bee",
                    look("minecraft:bee#main", Some(bee), Some("bee/bee_baby.png")),
                ),
                (
                    "minecraft:cow",
                    look("minecraft:cow#main", Some("cow/cow.png"), None),
                ),
                (
                    "minecraft:pig",
                    look("minecraft:pig#main", Some("pig/pig.png"), None),
                ),
                ("minecraft:zombified_piglin", piglin),
            ]),
        ),
    ];
    // 1.20.1's pack has the bee and the cow, but no baby bee.
    let has = |version: &str, texture: &str| {
        version != "1.20.1"
            || [bee, "cow/cow.png", "piglin/zombified_piglin.png"].contains(&texture)
    };
    borrow_textures(&mut dumps, has);
    let texture = |version: usize, id: &str| {
        let look = &dumps[version].1.entities[id];
        (look.texture.as_deref(), look.baby_texture.as_deref())
    };
    assert_eq!(
        texture(1, "minecraft:bee"),
        (Some(bee), Some("bee/bee_baby.png"))
    );
    assert_eq!(
        texture(0, "minecraft:bee"),
        (Some(bee), None),
        "no baby bee in 1.20.1"
    );
    assert_eq!(texture(0, "minecraft:cow"), (Some("cow/cow.png"), None));
    assert_eq!(
        texture(0, "minecraft:pig"),
        (None, None),
        "another main layer lends nothing"
    );
    // A renderer that could not be built takes the newer look, with only
    // the layers its version has.
    let piglin = &dumps[0].1.entities["minecraft:zombified_piglin"];
    assert_eq!(
        piglin.texture.as_deref(),
        Some("piglin/zombified_piglin.png")
    );
    assert_eq!(piglin.layers, ["minecraft:zombified_piglin#main"]);
    assert_eq!(piglin.error, None);
    assert!(piglin.is_living());

    let borrowed = borrowed_looks(&dumps[0].1, "1.16.5", |_, texture| texture == bee);
    assert_eq!(borrowed.keys().collect::<Vec<_>>(), ["minecraft:bee"]);
}

/// T35: a renderer that drew its model with code (arrows before 1.21.2)
/// takes the layer its newer version draws the same model from, geometry
/// and all; another renderer class lends nothing, and a layer the version
/// has is never replaced. A renderer that could not be built takes the
/// nearest built version's classes, and still counts as unbuilt for its
/// textures.
#[test]
fn code_drawn_renderers_borrow_layers_and_unbuilt_ones_their_classes() {
    let look = |renderer: &[&str], layers: &[&str]| EntityLook {
        renderer: renderer.iter().map(|c| (*c).to_owned()).collect(),
        layers: layers.iter().map(|l| (*l).to_owned()).collect(),
        ..EntityLook::default()
    };
    let unbuilt = EntityLook {
        error: Some("NullPointerException building the renderer".into()),
        ..EntityLook::default()
    };
    let dump = |looks: Vec<(&str, EntityLook)>, layers: &[&str]| EntityDump {
        format: DUMP_FORMAT,
        layers: layers
            .iter()
            .map(|layer| {
                (
                    (*layer).to_owned(),
                    EntityLayer {
                        texture_size: [32, 32],
                        root: root(Vec::new()),
                    },
                )
            })
            .collect(),
        entities: looks
            .into_iter()
            .map(|(id, look)| (id.to_owned(), look))
            .collect(),
        entities_error: None,
    };
    let arrow = ["TippableArrowRenderer", "ArrowRenderer"];
    let mut zombie = look(
        &["ZombieRenderer", "LivingEntityRenderer"],
        &["minecraft:zombie#main"],
    );
    zombie.texture = Some("zombie/zombie.png".into());
    let mut dumps = vec![
        (
            "1.20.1".to_owned(),
            dump(
                vec![
                    ("minecraft:arrow", look(&arrow, &[])),
                    ("minecraft:spectral_arrow", look(&["OldArrowRenderer"], &[])),
                    ("minecraft:painting", look(&["PaintingRenderer"], &[])),
                    ("minecraft:zombie", unbuilt.clone()),
                ],
                &["minecraft:zombie#main"],
            ),
        ),
        (
            "1.21.4".to_owned(),
            dump(
                vec![
                    ("minecraft:arrow", look(&arrow, &["minecraft:arrow#main"])),
                    (
                        "minecraft:spectral_arrow",
                        look(
                            &["SpectralArrowRenderer", "ArrowRenderer"],
                            &["minecraft:arrow#main"],
                        ),
                    ),
                    ("minecraft:painting", look(&["PaintingRenderer"], &[])),
                    ("minecraft:zombie", zombie),
                ],
                &["minecraft:arrow#main", "minecraft:zombie#main"],
            ),
        ),
        (
            "26.3".to_owned(),
            dump(vec![("minecraft:painting", unbuilt)], &[]),
        ),
    ];
    borrow_layers(&mut dumps);
    let older = &dumps[0].1;
    assert_eq!(
        older.entities["minecraft:arrow"].layers,
        ["minecraft:arrow#main"]
    );
    assert!(
        older.layers.contains_key("minecraft:arrow#main"),
        "its geometry too"
    );
    assert!(
        older.entities["minecraft:spectral_arrow"].layers.is_empty(),
        "another renderer lends nothing"
    );
    let painting = &dumps[2].1.entities["minecraft:painting"];
    assert_eq!(painting.renderer, ["PaintingRenderer"]);
    assert!(painting.error.is_some(), "still not built");

    borrow_textures(&mut dumps, |_, _| true);
    let zombie = &dumps[0].1.entities["minecraft:zombie"];
    assert_eq!(zombie.renderer, ["ZombieRenderer", "LivingEntityRenderer"]);
    assert_eq!(
        zombie.texture.as_deref(),
        Some("zombie/zombie.png"),
        "an unbuilt look takes the newer look whole, borrowed classes or not"
    );
}
