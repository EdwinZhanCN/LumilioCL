//! Manual check of the schematic studio's writes on real schematics (plan
//! schematic-studio, T19): each `.litematic` file is copied into a fresh
//! launcher home and, through the same service calls the studio makes,
//! replaced (its most numerous block becomes its second), converted to
//! 1.20.1 and saved beside itself, and both writes restored.
//!
//! Ignored by default. Run with
//! `LUMILIO_LIVE_SCHEMATICS=<folder> cargo nextest run -p lumilio-core --test schematic_live --run-ignored only --no-capture`;
//! `LUMILIO_LIVE_OUT=<folder>` keeps the replaced and converted files, to
//! open them in Litematica.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lumilio_core::{DefaultTransport, LauncherService, Loader, SchematicOperation};
use lumilio_plugin_litematica::{ID, Litematica};
use lumilio_schematic::replace::{Mapping, MappingRow, Scope, StateMatcher};
use lumilio_schematic::stats::{SchematicStats, stats};

/// 1.20.1's data version.
const DATA_1_20_1: i32 = 3465;

fn count_of(stats: &SchematicStats, id: &str) -> u64 {
    stats
        .blocks
        .iter()
        .find(|block| block.id == id)
        .map_or(0, |block| block.count)
}

fn files_in(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "litematic"))
        .collect();
    files.sort();
    files
}

#[tokio::test]
#[ignore = "reads schematics from LUMILIO_LIVE_SCHEMATICS"]
async fn real_schematics_survive_replace_convert_and_restore() {
    let Ok(folder) = std::env::var("LUMILIO_LIVE_SCHEMATICS") else {
        println!("no LUMILIO_LIVE_SCHEMATICS; nothing to check");
        return;
    };
    let out = std::env::var("LUMILIO_LIVE_OUT").ok().map(PathBuf::from);
    let home = tempfile::tempdir().unwrap();
    let service = LauncherService::open(
        home.path(),
        DefaultTransport::new().unwrap(),
        vec![Arc::new(Litematica)],
    )
    .unwrap();
    let instance = service
        .create_instance("Live", Some("1.20.1"), Loader::Vanilla, None)
        .await
        .unwrap()
        .id;
    let game = service.layout().game(&instance);
    std::fs::create_dir_all(game.join("schematics")).unwrap();

    let files = files_in(Path::new(&folder));
    assert!(!files.is_empty(), "no .litematic files in {folder}");
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let stem = name.trim_end_matches(".litematic").to_owned();
        let file = format!("schematics/{name}");
        let original = std::fs::read(&path).unwrap();
        std::fs::write(game.join(&file), &original).unwrap();

        // Replace, as the replace dialog writes it.
        let (bytes, info) = service.schematic_read(&instance, ID, &file).await.unwrap();
        let mut schematic = lumilio_schematic::load_litematic(&bytes).unwrap();
        let before = stats(&schematic);
        let (Some(most), Some(next)) = (before.blocks.first(), before.blocks.get(1)) else {
            println!("{name}: fewer than two kinds of block; skipped");
            continue;
        };
        let (from, to) = (most.id.clone(), next.id.clone());
        let mapping = Mapping {
            rows: vec![MappingRow {
                source: StateMatcher::any_state(from.clone()),
                target: to.clone(),
                fixed: Vec::new(),
            }],
            scope: Scope::All,
        };
        let changed = lumilio_schematic::replace::apply_mapping(&mut schematic, &mapping, None);
        assert!(changed > 0, "{name}: nothing replaced");
        let replaced = lumilio_schematic::to_litematic_bytes(&schematic).unwrap();
        let replace = service
            .schematic_apply(
                &instance,
                ID,
                &file,
                replaced,
                Some(info),
                SchematicOperation {
                    kind: "replace".into(),
                    summary: format!("{from} → {to}"),
                },
            )
            .await
            .unwrap();
        let written = std::fs::read(game.join(&file)).unwrap();
        let after = stats(&lumilio_schematic::load_litematic(&written).unwrap());
        assert_eq!(count_of(&after, &from), 0, "{name}: {from} is gone");
        assert_eq!(
            count_of(&after, &to),
            count_of(&before, &from) + count_of(&before, &to),
            "{name}: every {from} became {to}"
        );
        assert_eq!(after.block_count, before.block_count, "{name}");
        assert_eq!(after.regions.len(), before.regions.len(), "{name}");
        assert_eq!(after.entities, before.entities, "{name}");

        // Convert to 1.20.1 and save beside, as the convert dialog does.
        let mut converted = lumilio_schematic::load_litematic(&written).unwrap();
        let conversion = lumilio_schematic::convert::convert(&mut converted, DATA_1_20_1, None);
        let beside = format!("schematics/{stem} (1.20.1).litematic");
        let convert = service
            .schematic_apply(
                &instance,
                ID,
                &beside,
                lumilio_schematic::to_litematic_bytes(&converted).unwrap(),
                None,
                SchematicOperation {
                    kind: "convert".into(),
                    summary: "1.20.1".into(),
                },
            )
            .await
            .unwrap();
        let saved = std::fs::read(game.join(&beside)).unwrap();
        let reread = lumilio_schematic::load_litematic(&saved).unwrap();
        assert_eq!(
            lumilio_schematic::convert::data_version(&reread),
            DATA_1_20_1,
            "{name}"
        );
        assert_eq!(stats(&reread).regions.len(), before.regions.len(), "{name}");
        if let Some(out) = &out {
            std::fs::create_dir_all(out).unwrap();
            std::fs::write(out.join(format!("{stem} (replaced).litematic")), &written).unwrap();
            std::fs::write(out.join(format!("{stem} (1.20.1).litematic")), &saved).unwrap();
        }

        // Restore both: the new file goes to the backups, the original
        // comes back byte for byte.
        service
            .schematic_restore(&instance, ID, convert.index)
            .await
            .unwrap();
        assert!(
            !game.join(&beside).exists(),
            "{name}: the new file is backed up"
        );
        service
            .schematic_restore(&instance, ID, replace.index)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(game.join(&file)).unwrap(),
            original,
            "{name}: restored exactly"
        );
        println!(
            "{name}: {changed} × {from} → {to}; 1.20.1 with {} losses; both restored",
            conversion.losses.len()
        );
    }
    let history = service.schematic_history(&instance).await.unwrap();
    println!("{} history lines", history.len());
}
