use std::sync::Arc;

use lumilio_plugin_api::{API_VERSION, Manifest, Permission, Plugin, PluginError, Words};

use super::Scripted;
use crate::instance::Loader;
use crate::schematic_ops::RESTORE;
use crate::{LauncherService, SchematicOperation, ServiceError};

const PLUGIN: &str = "test.schematics";
const FILE: &str = "schematics/house.litematic";

/// Declares what the Litematica plugin declares, and nothing else.
struct Schematics;

impl Plugin for Schematics {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: PLUGIN.into(),
            name: Words::new("S", "S"),
            description: Words::default(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: vec![
                Permission::ReadGameFiles {
                    under: "schematics".into(),
                },
                Permission::WriteGameFiles {
                    under: "schematics".into(),
                    names: "*.litematic".into(),
                },
            ],
            settings: vec![],
        }
    }
}

struct Studio {
    _dir: tempfile::TempDir,
    service: LauncherService<Scripted>,
    instance: String,
}

async fn studio() -> Studio {
    let dir = tempfile::tempdir().unwrap();
    let service = LauncherService::open(
        dir.path().join("launcher"),
        Scripted::default(),
        vec![Arc::new(Schematics)],
    )
    .unwrap();
    let instance = service
        .create_instance("Builds", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap()
        .id;
    let file = service.layout().game(&instance).join(FILE);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"original").unwrap();
    Studio {
        _dir: dir,
        service,
        instance,
    }
}

fn replace() -> SchematicOperation {
    SchematicOperation {
        kind: "replace".into(),
        summary: "stone -> andesite".into(),
    }
}

#[tokio::test]
async fn a_running_game_keeps_every_schematic_write_out() {
    let studio = studio().await;
    let (service, instance) = (&studio.service, studio.instance.as_str());
    let (_, info) = service
        .schematic_read(instance, PLUGIN, FILE)
        .await
        .unwrap();
    assert!(service.schematic_can_write(instance));
    {
        // A running game holds this same lease.
        let _running = service.reserve_instance(instance).unwrap();
        assert!(!service.schematic_can_write(instance));
        let refused = service
            .schematic_apply(
                instance,
                PLUGIN,
                FILE,
                b"edited".to_vec(),
                Some(info.clone()),
                replace(),
            )
            .await;
        assert!(
            matches!(refused, Err(ServiceError::InstanceBusy(_))),
            "{refused:?}"
        );
        assert!(matches!(
            service.schematic_restore(instance, PLUGIN, 1).await,
            Err(ServiceError::InstanceBusy(_))
        ));
    }
    let game_file = service.layout().game(instance).join(FILE);
    assert_eq!(std::fs::read(&game_file).unwrap(), b"original");
    assert!(
        service
            .schematic_history(instance)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(!service.layout().schematic_ops(instance).exists());
    assert!(
        service.schematic_can_write(instance),
        "the lease is released"
    );
}

#[tokio::test]
async fn an_edit_is_backed_up_in_launcher_data_listed_and_restorable() {
    let studio = studio().await;
    let (service, instance) = (&studio.service, studio.instance.as_str());
    let game_file = service.layout().game(instance).join(FILE);
    let (bytes, info) = service
        .schematic_read(instance, PLUGIN, FILE)
        .await
        .unwrap();
    assert_eq!(bytes, b"original");
    let entry = service
        .schematic_apply(
            instance,
            PLUGIN,
            FILE,
            b"edited".to_vec(),
            Some(info),
            replace(),
        )
        .await
        .unwrap();
    assert_eq!(std::fs::read(&game_file).unwrap(), b"edited");
    let backup = service
        .layout()
        .schematic_ops(instance)
        .join(entry.backup.as_deref().unwrap());
    assert!(!backup.starts_with(service.layout().game(instance)));
    assert_eq!(std::fs::read(&backup).unwrap(), b"original");

    let restored = service
        .schematic_restore(instance, PLUGIN, entry.index)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&game_file).unwrap(), b"original");
    let history = service.schematic_history(instance).await.unwrap();
    assert_eq!(history, vec![restored.clone(), entry]);
    assert_eq!(restored.operation, RESTORE);
}

#[tokio::test]
async fn writes_stay_inside_the_plugins_grant() {
    let studio = studio().await;
    let (service, instance) = (&studio.service, studio.instance.as_str());
    let (_, info) = service
        .schematic_read(instance, PLUGIN, FILE)
        .await
        .unwrap();
    for (plugin, file) in [
        (PLUGIN, "schematics/house.txt"),
        (PLUGIN, "options.txt"),
        (PLUGIN, "schematics/../options.litematic"),
        ("test.missing", FILE),
    ] {
        let refused = service
            .schematic_apply(
                instance,
                plugin,
                file,
                b"x".to_vec(),
                Some(info.clone()),
                replace(),
            )
            .await;
        assert!(
            matches!(
                refused,
                Err(ServiceError::Plugin(
                    PluginError::PermissionDenied | PluginError::InvalidInput(_)
                ))
            ),
            "{plugin} {file}: {refused:?}"
        );
    }
    let off = lumilio_plugin_api::PluginState {
        enabled: Some(false),
        ..Default::default()
    };
    service.plugins.set_state(PLUGIN.into(), off);
    assert!(matches!(
        service
            .schematic_apply(instance, PLUGIN, FILE, b"x".to_vec(), Some(info), replace())
            .await,
        Err(ServiceError::Plugin(PluginError::PermissionDenied))
    ));
    assert!(
        service
            .schematic_history(instance)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn removing_is_a_logged_move_refused_while_the_game_runs() {
    let studio = studio().await;
    let (service, instance) = (&studio.service, studio.instance.as_str());
    let game_file = service.layout().game(instance).join(FILE);
    let (_, info) = service
        .schematic_read(instance, PLUGIN, FILE)
        .await
        .unwrap();
    let delete = SchematicOperation {
        kind: "delete".into(),
        summary: String::new(),
    };
    {
        let _running = service.reserve_instance(instance).unwrap();
        let refused = service
            .schematic_remove(
                instance,
                PLUGIN,
                vec![(FILE.into(), info.clone())],
                delete.clone(),
            )
            .await;
        assert!(
            matches!(refused, Err(ServiceError::InstanceBusy(_))),
            "{refused:?}"
        );
        assert!(game_file.exists());
    }
    let outside = service
        .schematic_remove(
            instance,
            PLUGIN,
            vec![("options.txt".into(), info.clone())],
            delete.clone(),
        )
        .await;
    assert!(outside.is_err(), "only files the plugin may write");

    let entries = service
        .schematic_remove(instance, PLUGIN, vec![(FILE.into(), info)], delete)
        .await
        .unwrap();
    assert!(!game_file.exists());
    assert_eq!(service.schematic_history(instance).await.unwrap(), entries);
    service
        .schematic_restore(instance, PLUGIN, entries[0].index)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&game_file).unwrap(), b"original");
}
