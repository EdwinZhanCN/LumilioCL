use super::naming::*;
use super::overlay;
use super::waypoints::*;
use crate::WorldExplorer;
use lumilio_plugin_api::map::{
    Dimension, MapBounds, MapIcon, MapObjectKind, MapPoint, OverlayProvider, OverlayRequest,
    SourceLink, WorldContext, WorldId,
};
use lumilio_plugin_api::{FetchResponse, HostContext, PluginError, SettingValue};
use std::collections::BTreeMap;

const OVERWORLD: &str = include_str!("../../tests/data/xaero/synthetic/dim%0/mw$default_1.txt");
const NETHER: &str = include_str!("../../tests/data/xaero/synthetic/dim%-1/mw$default_1.txt");
const END: &str = include_str!("../../tests/data/xaero/synthetic/dim%1/mw$default_1.txt");

#[test]
fn files_round_trip_byte_for_byte_including_comments_unknown_lines_and_crlf() {
    for text in [
        OVERWORLD,
        NETHER,
        END,
        "",
        "no trailing newline",
        "\n\n#x\r\n",
    ] {
        let file = WaypointFile::parse(text);
        assert_eq!(file.text(), text);
    }
    let file = WaypointFile::parse(OVERWORLD);
    assert_eq!(file.malformed(), 0);
    let waypoints: Vec<&Waypoint> = file
        .lines()
        .iter()
        .filter_map(|line| line.waypoint.as_ref())
        .collect();
    assert_eq!(waypoints.len(), 5);
    assert_eq!(waypoints[0].name, "Home base");
    assert_eq!(
        (waypoints[0].x, waypoints[0].y, waypoints[0].z),
        (100, Some(64), -250)
    );
    assert_eq!(
        waypoints[1].name, "Nether: portal",
        "the escaped colon is restored"
    );
    assert_eq!(waypoints[1].y, None, "~ means no Y");
    assert!(waypoints[1].rotate_on_tp && waypoints[1].tp_yaw == 90);
    assert_eq!(waypoints[2].name, "StashFinder - 🪧 8");
    assert_eq!(waypoints[2].initials, "🪧");
    assert_eq!(waypoints[2].set, "farms");
    assert!(waypoints[3].is_death() && waypoints[4].is_death());
    assert!(waypoints[4].disabled);
    // The line the mod's writer would produce is the line we read.
    for line in file.lines().iter().filter(|line| line.waypoint.is_some()) {
        let parsed = line.waypoint.as_ref().unwrap();
        assert!(OVERWORLD.contains(&format_line(parsed)), "{parsed:?}");
    }
}

#[test]
fn a_waypoint_that_does_not_parse_is_kept_and_flagged() {
    let text = "waypoint:broken:b:notanumber:64:0:1:false:0:gui.xaero_default\nwaypoint:short:1\n";
    let file = WaypointFile::parse(text);
    assert_eq!(file.malformed(), 2);
    assert_eq!(file.text(), text);
}

#[test]
fn replacing_a_waypoint_changes_only_its_line_and_keeps_the_ending() {
    let mut file = WaypointFile::parse(NETHER);
    let mut waypoint = file.lines()[4].waypoint.clone().unwrap();
    waypoint.name = "Fortress: east".into();
    waypoint.y = Some(31);
    file.replace(4, waypoint).unwrap();
    let before: Vec<&str> = NETHER.split_inclusive('\n').collect();
    let after_text = file.text();
    let after: Vec<&str> = after_text.split_inclusive('\n').collect();
    assert_eq!(before[..4], after[..4], "other lines are untouched");
    assert_eq!(
        after[4],
        "waypoint:Fortress§§ east:F:-120:31:80:14:false:0:gui.xaero_default:false:0:0:false\r\n"
    );
    assert!(
        file.replace(
            0,
            Waypoint::clone(file.lines()[4].waypoint.as_ref().unwrap())
        )
        .is_none(),
        "a comment is not a waypoint"
    );
}

#[test]
fn colours_are_the_sixteen_text_colours_then_the_four_extra_ones() {
    assert_eq!(color_rgb(0), [0, 0, 0]);
    assert_eq!(color_rgb(15), [255, 255, 255]);
    assert_eq!(color_rgb(19), [0xF3, 0x8B, 0xAA]);
    assert_eq!(
        color_rgb(200),
        [0xAA, 0xAA, 0xAA],
        "unknown indices show grey"
    );
    assert_eq!(CHOOSABLE_COLORS, 16);
}

#[test]
fn share_strings_escape_colons_and_name_the_dimension() {
    let file = WaypointFile::parse(OVERWORLD);
    let waypoints: Vec<&Waypoint> = file
        .lines()
        .iter()
        .filter_map(|line| line.waypoint.as_ref())
        .collect();
    assert_eq!(
        share_string(waypoints[0], &Dimension::Overworld),
        "xaero-waypoint:Home base:H:100:64:-250:10:false:0:Internal-overworld-waypoints"
    );
    assert_eq!(
        share_string(waypoints[1], &Dimension::Nether),
        "xaero-waypoint:Nether§§ portal:N:12:~:-40:12:true:90:Internal-the-nether-waypoints"
    );
    assert!(
        share_string(waypoints[2], &Dimension::End)
            .starts_with("xaero-waypoint:StashFinder - 🪧 8:🪧:-24:64:20999928:6:")
    );
    assert!(share_string(waypoints[0], &Dimension::End).ends_with(":Internal-the-end-waypoints"));
}

#[test]
fn dimension_folders_and_file_names_follow_the_mods_conventions() {
    assert_eq!(dimension_of("dim%0"), Some(Dimension::Overworld));
    assert_eq!(dimension_of("dim%-1"), Some(Dimension::Nether));
    assert_eq!(dimension_of("dim%1"), Some(Dimension::End));
    assert_eq!(dimension_of("The End"), Some(Dimension::End));
    assert_eq!(
        dimension_of("dim%minecraft$worlds%a%b"),
        Some(Dimension::Custom("minecraft:worlds/a/b".into()))
    );
    assert_eq!(dimension_of("notes"), None);
    for dimension in [
        Dimension::Overworld,
        Dimension::Nether,
        Dimension::End,
        Dimension::Custom("mod:deep/er".into()),
    ] {
        assert_eq!(dimension_of(&folder_of(&dimension)), Some(dimension));
    }
    assert!(is_backup_dir("backup") && is_backup_dir("backup--") && !is_backup_dir("backups"));
    assert!(is_transient("mw$default_1.txt.temp") && is_transient("mw$default_1.txt.backup3"));
    assert!(!is_transient("mw$default_1.txt"));
    assert_eq!(
        parse_file_name("mw$default_1.txt"),
        Some((Some("mw$default"), "1".to_owned()))
    );
    assert_eq!(
        parse_file_name("waypoints.txt"),
        Some((None, "waypoints".into()))
    );
    assert_eq!(parse_file_name("mw$a_my%us%base.txt").unwrap().1, "my_base");
    assert_eq!(parse_file_name("mw$default_1.json"), None);
    let root = "xaero/minimap/Multiplayer_x/dim%0";
    assert!(waypoint_file(&format!("{root}/mw$default_1.txt"), "Multiplayer_x").is_some());
    assert!(waypoint_file(&format!("{root}/mw$default_1.txt.temp"), "Multiplayer_x").is_none());
    assert!(
        waypoint_file(
            "xaero/minimap/Multiplayer_x/dim%0/backup/mw$d_1.txt",
            "Multiplayer_x"
        )
        .is_none()
    );
    assert!(waypoint_file(&format!("{root}/mw$default_1.txt"), "Other").is_none());
    assert!(
        waypoint_file(
            "xaero/minimap/Multiplayer_x/mw$default_1.txt",
            "Multiplayer_x"
        )
        .is_none()
    );
}

/// A game directory in memory: path to bytes.
struct Files(BTreeMap<String, Vec<u8>>);
impl HostContext for Files {
    fn setting(&self, _: &str) -> Option<SettingValue> {
        None
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError> {
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| PluginError::Unavailable("missing".into()))
    }
    fn list_files(&self, dir: &str) -> Result<Vec<String>, PluginError> {
        Ok(self
            .0
            .keys()
            .filter(|path| path.starts_with(&format!("{dir}/")))
            .cloned()
            .collect())
    }
    fn fetch(&self, _: &str) -> Result<FetchResponse, PluginError> {
        panic!("waypoints never use the network")
    }
}

fn game(dir: &str) -> Files {
    let mut files = BTreeMap::new();
    for (folder, text) in [("dim%0", OVERWORLD), ("dim%-1", NETHER), ("dim%1", END)] {
        files.insert(
            format!("xaero/minimap/{dir}/{folder}/mw$default_1.txt"),
            text.as_bytes().to_vec(),
        );
        files.insert(
            format!("xaero/minimap/{dir}/{folder}/mw$default_1.txt.temp"),
            b"waypoint:Half written:h:1:1:1:1:false:0:gui.xaero_default:false:0:0:false\n".to_vec(),
        );
        files.insert(
            format!("xaero/minimap/{dir}/{folder}/backup/mw$default_1.txt"),
            b"waypoint:Deleted since:d:2:2:2:1:false:0:gui.xaero_default:false:0:0:false\n"
                .to_vec(),
        );
    }
    Files(files)
}

fn linked(dir: &str, dimension: Dimension) -> WorldContext {
    WorldContext {
        world: WorldId::Save {
            instance: "i".into(),
            folder: "World".into(),
        },
        version: Some("1.21.4".into()),
        data_version: None,
        seed: Some(1),
        dimension,
        sources: vec![
            SourceLink::Save("World".into()),
            SourceLink::XaeroMinimap(dir.into()),
        ],
    }
}

fn request(context: WorldContext, area: [f64; 4]) -> OverlayRequest {
    OverlayRequest {
        context,
        overlay: overlay::LAYER.into(),
        bounds: MapBounds {
            min: MapPoint {
                x: area[0],
                z: area[1],
            },
            max: MapPoint {
                x: area[2],
                z: area[3],
            },
        },
        level: 0,
    }
}

#[test]
fn the_layer_exists_only_for_a_linked_world_and_reads_only_live_files() {
    let unlinked = WorldContext {
        sources: vec![SourceLink::Save("World".into())],
        ..linked("x", Dimension::Overworld)
    };
    let has = |context: &WorldContext| {
        WorldExplorer
            .overlays_for(context)
            .iter()
            .any(|layer| layer.id == overlay::LAYER)
    };
    assert!(!has(&unlinked));
    assert!(has(&linked("Multiplayer_a", Dimension::Overworld)));

    let files = game("Multiplayer_a");
    let found = WorldExplorer
        .objects(
            &files,
            &request(
                linked("Multiplayer_a", Dimension::Overworld),
                [-1000., -1000., 1000., 1000.],
            ),
        )
        .unwrap();
    let names: Vec<_> = found.iter().filter_map(|o| o.label.clone()).collect();
    assert_eq!(
        names,
        [
            "Home base",
            "Nether: portal",
            "gui.xaero_deathpoint",
            "Old grave"
        ],
        "the .temp and backup files are skipped; the far StashFinder is outside the box"
    );
    let home = &found[0];
    assert_eq!(home.color, Some(color_rgb(10)));
    assert_eq!(home.label_id.as_deref(), Some("map-xaero-waypoint"));
    assert!(matches!(
        home.kind,
        MapObjectKind::Icon {
            icon: MapIcon::Waypoint,
            at: MapPoint { x: 100., z: -250. }
        }
    ));
    assert!(
        home.share
            .as_deref()
            .unwrap()
            .starts_with("xaero-waypoint:Home base:H:100:64:-250:")
    );
    assert_eq!(home.note, None, "the default set is not worth a line");
    let death = &found[2];
    assert!(matches!(
        death.kind,
        MapObjectKind::Icon {
            icon: MapIcon::Death,
            ..
        }
    ));
    assert_eq!(death.label_id.as_deref(), Some("map-xaero-death"));
    let grave = &found[3];
    assert!(
        grave.priority < home.priority,
        "a disabled point sits under live ones"
    );
    // A second set is named on the card.
    let stash = WorldExplorer
        .objects(
            &files,
            &request(
                linked("Multiplayer_a", Dimension::Overworld),
                [-100., 20_999_000., 100., 21_000_000.],
            ),
        )
        .unwrap();
    assert_eq!(stash[0].note.as_deref(), Some("farms"));
    // The nether file is a different dimension, with its own CRLF line.
    let nether = WorldExplorer
        .objects(
            &files,
            &request(
                linked("Multiplayer_a", Dimension::Nether),
                [-500., -500., 500., 500.],
            ),
        )
        .unwrap();
    assert_eq!(nether.len(), 1);
    assert_eq!(nether[0].label.as_deref(), Some("Fortress"));
}

#[test]
fn damaged_waypoint_files_make_the_layer_unavailable_instead_of_partial() {
    let mut files = game("Multiplayer_b");
    files.0.insert(
        "xaero/minimap/Multiplayer_b/dim%0/mw$default_1.txt".into(),
        b"waypoint:oops:o:x:y:z\n".to_vec(),
    );
    let asked = request(
        linked("Multiplayer_b", Dimension::Overworld),
        [-10., -10., 10., 10.],
    );
    assert_eq!(
        WorldExplorer.objects(&files, &asked),
        Err(PluginError::Unavailable("map-xaero-unreadable".into()))
    );
    files.0.insert(
        "xaero/minimap/Multiplayer_b/dim%0/mw$default_1.txt".into(),
        vec![0xff, 0xfe, 0x00],
    );
    // A different dimension key so the one-second reuse cannot hide the file.
    let asked = request(
        linked("Multiplayer_b", Dimension::End),
        [-10., -10., 10., 10.],
    );
    files.0.insert(
        "xaero/minimap/Multiplayer_b/dim%1/mw$default_1.txt".into(),
        vec![0xff, 0xfe, 0x00],
    );
    assert_eq!(
        WorldExplorer.objects(&files, &asked),
        Err(PluginError::Unavailable("map-xaero-unreadable".into()))
    );
}
