use super::world;
use crate::MapFailure;
use crate::instance::Loader;
use lumilio_plugin_api::map::{Dimension, EditAction, MapPoint, ObjectEdit, WorldContext, WorldId};
use std::collections::BTreeMap;

fn edit() -> ObjectEdit {
    ObjectEdit {
        context: WorldContext {
            world: WorldId::Seed {
                seed: 1,
                version: "1.21.4".into(),
            },
            version: None,
            data_version: None,
            seed: Some(1),
            dimension: Dimension::Overworld,
            sources: vec![],
        },
        overlay: "xaero.waypoints".into(),
        action: EditAction::Create {
            at: MapPoint { x: 0., z: 0. },
            values: BTreeMap::new(),
        },
    }
}

#[tokio::test]
async fn edits_wait_for_the_game_to_leave_and_hold_the_instance_while_they_run() {
    let world = world();
    let record = world
        .service
        .create_instance("Edits", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    assert!(world.service.map_can_edit(&record.id));
    {
        // A running game holds this same lease.
        let _running = world.service.reserve_instance(&record.id).unwrap();
        assert!(!world.service.map_can_edit(&record.id));
        assert_eq!(
            world
                .service
                .map_apply(&record.id, "lumilio.world-explorer", edit())
                .await,
            Err(MapFailure::Failed("map-edit-running".into()))
        );
    }
    assert!(
        world.service.map_can_edit(&record.id),
        "the lease is released"
    );
    // The probe above did not leave the instance held.
    assert!(world.service.reserve_instance(&record.id).is_ok());
}
