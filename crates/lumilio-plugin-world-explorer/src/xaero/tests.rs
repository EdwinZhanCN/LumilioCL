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

mod editing {
    use super::*;
    use crate::xaero::edit::{self, INITIALS_MAX, NAME_MAX};
    use lumilio_plugin_api::map::{EditAction, ObjectEdit};
    use lumilio_plugin_api::{FileInfo, SettingValue};
    use std::sync::Mutex;

    /// A game directory in memory that also takes writes, as the host would:
    /// a write needs the `FileInfo` of the last read.
    struct Disk {
        files: Mutex<BTreeMap<String, Vec<u8>>>,
        writes: Mutex<Vec<String>>,
    }
    impl Disk {
        fn new(dir: &str) -> Self {
            Self {
                files: Mutex::new(game(dir).0),
                writes: Mutex::new(vec![]),
            }
        }
        fn info(bytes: &[u8]) -> FileInfo {
            FileInfo {
                len: bytes.len() as u64,
                modified_ms: 1,
                sha256: format!(
                    "{}-{}",
                    bytes.len(),
                    bytes.iter().map(|b| u64::from(*b)).sum::<u64>()
                ),
            }
        }
        fn text(&self, path: &str) -> String {
            String::from_utf8(self.files.lock().unwrap()[path].clone()).unwrap()
        }
    }
    impl HostContext for Disk {
        fn setting(&self, _: &str) -> Option<SettingValue> {
            None
        }
        fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError> {
            self.files
                .lock()
                .unwrap()
                .get(path)
                .cloned()
                .ok_or_else(|| PluginError::Unavailable("missing".into()))
        }
        fn read_file_info(&self, path: &str) -> Result<(Vec<u8>, FileInfo), PluginError> {
            let bytes = self.read_file(path)?;
            let info = Self::info(&bytes);
            Ok((bytes, info))
        }
        fn list_files(&self, dir: &str) -> Result<Vec<String>, PluginError> {
            Ok(self
                .files
                .lock()
                .unwrap()
                .keys()
                .filter(|path| path.starts_with(&format!("{dir}/")))
                .cloned()
                .collect())
        }
        fn write_file(
            &self,
            path: &str,
            bytes: &[u8],
            expected: Option<&FileInfo>,
        ) -> Result<(), PluginError> {
            let mut files = self.files.lock().unwrap();
            match (files.get(path), expected) {
                (Some(old), Some(expected)) if &Self::info(old) == expected => {}
                (None, None) => {}
                _ => return Err(PluginError::Unavailable("map-edit-conflict".into())),
            }
            files.insert(path.to_owned(), bytes.to_vec());
            self.writes.lock().unwrap().push(path.to_owned());
            Ok(())
        }
        fn fetch(&self, _: &str) -> Result<FetchResponse, PluginError> {
            panic!("no network")
        }
    }

    fn values(pairs: &[(&str, SettingValue)]) -> BTreeMap<String, SettingValue> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect()
    }
    fn full(name: &str, x: i64) -> BTreeMap<String, SettingValue> {
        values(&[
            ("name", SettingValue::Text(name.into())),
            ("initials", SettingValue::Text(String::new())),
            ("x", SettingValue::Number(x)),
            ("has_y", SettingValue::Toggle(true)),
            ("y", SettingValue::Number(70)),
            ("z", SettingValue::Number(-5)),
            ("color", SettingValue::Choice("12".into())),
            ("enabled", SettingValue::Toggle(true)),
            ("rotate", SettingValue::Toggle(false)),
            ("yaw", SettingValue::Number(0)),
        ])
    }
    const FILE: &str = "xaero/minimap/Edit/dim%0/mw$default_1.txt";

    fn edit_of(dir: &str, dimension: Dimension, action: EditAction) -> ObjectEdit {
        ObjectEdit {
            context: linked(dir, dimension),
            overlay: overlay::LAYER.into(),
            action,
        }
    }
    fn home_id(disk: &Disk, dir: &str) -> (String, Vec<lumilio_plugin_api::SettingField>) {
        overlay::forget_recent();
        let found = WorldExplorer
            .objects(
                disk,
                &request(
                    linked(dir, Dimension::Overworld),
                    [-1000., -1000., 1000., 1000.],
                ),
            )
            .unwrap();
        (found[0].id.clone(), found[0].editable.clone())
    }

    #[test]
    fn updating_changes_one_line_and_keeps_what_the_dialog_does_not_show() {
        let disk = Disk::new("Edit");
        let before = disk.text(FILE);
        let (id, fields) = home_id(&disk, "Edit");
        assert!(fields.iter().any(|f| f.key == "color"));
        WorldExplorer
            .apply(
                &disk,
                &edit_of(
                    "Edit",
                    Dimension::Overworld,
                    EditAction::Update {
                        id,
                        values: full("Base: new", 101),
                    },
                ),
            )
            .unwrap();
        let after = disk.text(FILE);
        let (old, new): (Vec<&str>, Vec<&str>) = (
            before.split_inclusive('\n').collect(),
            after.split_inclusive('\n').collect(),
        );
        assert_eq!(old.len(), new.len());
        let changed: Vec<usize> = (0..old.len()).filter(|i| old[*i] != new[*i]).collect();
        assert_eq!(changed, [4], "only the edited line differs");
        assert_eq!(
            new[4],
            "waypoint:Base§§ new:B:101:70:-5:12:false:0:gui.xaero_default:false:0:0:false\n"
        );
        // A death point stays a death point and a farms-set point stays in farms.
        let (death, _) = {
            overlay::forget_recent();
            let found = WorldExplorer
                .objects(
                    &disk,
                    &request(linked("Edit", Dimension::Overworld), [-10., -10., 10., 10.]),
                )
                .unwrap();
            let death = found
                .iter()
                .find(|o| o.label_id.as_deref() == Some("map-xaero-death"))
                .unwrap();
            (death.id.clone(), ())
        };
        WorldExplorer
            .apply(
                &disk,
                &edit_of(
                    "Edit",
                    Dimension::Overworld,
                    EditAction::Update {
                        id: death,
                        values: full("Grave", 5),
                    },
                ),
            )
            .unwrap();
        assert!(
            disk.text(FILE).contains(
                "waypoint:Grave:G:5:70:-5:12:false:1:gui.xaero_default:false:0:0:false\n"
            )
        );
    }

    #[test]
    fn invalid_values_are_refused_before_anything_is_written() {
        let disk = Disk::new("Edit");
        let (id, _) = home_id(&disk, "Edit");
        let bad = |change: &dyn Fn(&mut BTreeMap<String, SettingValue>)| {
            let mut map = full("Fine", 1);
            change(&mut map);
            WorldExplorer.apply(
                &disk,
                &edit_of(
                    "Edit",
                    Dimension::Overworld,
                    EditAction::Update {
                        id: id.clone(),
                        values: map,
                    },
                ),
            )
        };
        let long = "x".repeat(NAME_MAX + 1);
        assert_eq!(
            bad(&|m| {
                m.insert("name".into(), SettingValue::Text(long.clone()));
            }),
            Err(PluginError::Unavailable("map-edit-name-invalid".into()))
        );
        assert!(
            bad(&|m| {
                m.insert("name".into(), SettingValue::Text("a\nb".into()));
            })
            .is_err()
        );
        assert!(
            bad(&|m| {
                m.insert("name".into(), SettingValue::Text("  ".into()));
            })
            .is_err()
        );
        assert_eq!(
            bad(&|m| {
                m.insert(
                    "initials".into(),
                    SettingValue::Text("A".repeat(INITIALS_MAX + 1)),
                );
            }),
            Err(PluginError::Unavailable("map-edit-initials-invalid".into()))
        );
        // Colours past 15 are not offered, out-of-range numbers and wrong kinds are not accepted.
        assert!(
            bad(&|m| {
                m.insert("color".into(), SettingValue::Choice("16".into()));
            })
            .is_err()
        );
        assert!(
            bad(&|m| {
                m.insert("x".into(), SettingValue::Number(40_000_000));
            })
            .is_err()
        );
        assert!(
            bad(&|m| {
                m.insert("yaw".into(), SettingValue::Number(10_000));
            })
            .is_err()
        );
        assert!(
            bad(&|m| {
                m.insert("x".into(), SettingValue::Text("1".into()));
            })
            .is_err()
        );
        assert!(
            bad(&|m| {
                m.remove("z");
            })
            .is_err()
        );
        assert!(disk.writes.lock().unwrap().is_empty());
    }

    #[test]
    fn a_changed_file_or_line_refuses_the_edit() {
        let disk = Disk::new("Edit");
        let (id, _) = home_id(&disk, "Edit");
        // The game rewrote the file in between: the same line index now holds another waypoint.
        let moved = disk.text(FILE).replacen("Home base", "Someone else", 1);
        disk.files
            .lock()
            .unwrap()
            .insert(FILE.into(), moved.clone().into_bytes());
        assert_eq!(
            WorldExplorer.apply(
                &disk,
                &edit_of(
                    "Edit",
                    Dimension::Overworld,
                    EditAction::Update {
                        id: id.clone(),
                        values: full("Mine", 1)
                    }
                )
            ),
            Err(PluginError::Unavailable("map-edit-stale".into()))
        );
        assert_eq!(
            WorldExplorer.apply(
                &disk,
                &edit_of("Edit", Dimension::Overworld, EditAction::Delete { id })
            ),
            Err(PluginError::Unavailable("map-edit-stale".into()))
        );
        assert_eq!(disk.text(FILE), moved);
        // Comment lines and unknown ids are never edited.
        for id in [
            "xaero.waypoints:mw$default_1.txt:0:00000000",
            "xaero.waypoints:nonsense",
            "other:1",
        ] {
            assert!(
                WorldExplorer
                    .apply(
                        &disk,
                        &edit_of(
                            "Edit",
                            Dimension::Overworld,
                            EditAction::Delete { id: id.into() }
                        )
                    )
                    .is_err()
            );
        }
        assert!(disk.writes.lock().unwrap().is_empty());
    }

    #[test]
    fn deleting_and_creating_touch_only_their_own_line() {
        let disk = Disk::new("Edit");
        let before = disk.text(FILE);
        let (id, _) = home_id(&disk, "Edit");
        WorldExplorer
            .apply(
                &disk,
                &edit_of("Edit", Dimension::Overworld, EditAction::Delete { id }),
            )
            .unwrap();
        assert_eq!(
            disk.text(FILE),
            before.replacen(
                "waypoint:Home base:H:100:64:-250:10:false:0:gui.xaero_default:false:0:0:false\n",
                "",
                1
            )
        );
        // A new waypoint goes at the end of the dimension's file with the file's ending.
        let nether = "xaero/minimap/Edit/dim%-1/mw$default_1.txt";
        WorldExplorer
            .apply(
                &disk,
                &edit_of(
                    "Edit",
                    Dimension::Nether,
                    EditAction::Create {
                        at: MapPoint { x: 7., z: 9. },
                        values: full("Bastion", 7),
                    },
                ),
            )
            .unwrap();
        let text = disk.text(nether);
        assert!(text.starts_with(NETHER), "existing lines are untouched");
        assert!(
            text.ends_with(
                "waypoint:Bastion:B:7:70:-5:12:false:0:gui.xaero_default:false:0:0:false\r\n"
            ),
            "{text:?}"
        );
        // A dimension with no file gets a fresh one with the header.
        WorldExplorer
            .apply(
                &disk,
                &edit_of(
                    "Edit",
                    Dimension::Custom("mod:deep".into()),
                    EditAction::Create {
                        at: MapPoint { x: 0., z: 0. },
                        values: full("Deep", 0),
                    },
                ),
            )
            .unwrap();
        let fresh = disk.text("xaero/minimap/Edit/dim%mod$deep/mw$default_1.txt");
        assert!(
            fresh.starts_with("#\n#waypoint:name:")
                && fresh.contains("sets:gui.xaero_default\nwaypoint:Deep:")
        );
        // The layer offers creation; an unlinked world cannot be edited at all.
        let layer = WorldExplorer
            .overlays_for(&linked("Edit", Dimension::Overworld))
            .into_iter()
            .find(|layer| layer.id == overlay::LAYER)
            .unwrap();
        assert!(layer.creatable.iter().any(|f| f.key == "name"));
        let mut unlinked = edit_of(
            "Edit",
            Dimension::Overworld,
            EditAction::Delete { id: "x".into() },
        );
        unlinked.context.sources.clear();
        assert_eq!(
            WorldExplorer.apply(&disk, &unlinked),
            Err(PluginError::Unavailable("map-edit-unlinked".into()))
        );
    }

    #[test]
    fn a_damaged_file_is_never_rewritten() {
        let disk = Disk::new("Edit");
        let (id, _) = home_id(&disk, "Edit");
        let damaged = format!("{}waypoint:oops:o:x\n", disk.text(FILE));
        disk.files
            .lock()
            .unwrap()
            .insert(FILE.into(), damaged.clone().into_bytes());
        assert_eq!(
            WorldExplorer.apply(
                &disk,
                &edit_of("Edit", Dimension::Overworld, EditAction::Delete { id })
            ),
            Err(PluginError::Unavailable("map-xaero-unreadable".into()))
        );
        assert_eq!(
            WorldExplorer.apply(
                &disk,
                &edit_of(
                    "Edit",
                    Dimension::Overworld,
                    EditAction::Create {
                        at: MapPoint { x: 0., z: 0. },
                        values: full("N", 0)
                    }
                )
            ),
            Err(PluginError::Unavailable("map-xaero-unreadable".into()))
        );
        assert_eq!(disk.text(FILE), damaged);
    }

    #[test]
    fn waypoint_files_grow_and_shrink_without_disturbing_line_endings() {
        let mut file = WaypointFile::parse(
            "#c\r\nwaypoint:A:A:1:~:1:1:false:0:s:false:0:0:false\r\nlast-without-ending",
        );
        let waypoint = file.lines()[1].waypoint.clone().unwrap();
        file.push(waypoint.clone());
        assert_eq!(
            file.text(),
            "#c\r\nwaypoint:A:A:1:~:1:1:false:0:s:false:0:0:false\r\nlast-without-ending\r\nwaypoint:A:A:1:~:1:1:false:0:s:false:0:0:false\r\n"
        );
        assert_eq!(file.remove(1), Some(waypoint));
        assert!(file.remove(0).is_none(), "a comment is not removed");
        assert_eq!(file.remove(99), None);
        assert!(
            !file
                .text()
                .contains("waypoint:A:A:1:~:1:1:false:0:s:false:0:0:false\r\nlast")
        );
        assert_eq!(edit::fingerprint("a"), edit::fingerprint("a"));
        assert_ne!(edit::fingerprint("a"), edit::fingerprint("b"));
    }
}
