use super::*;

fn database() -> (tempfile::TempDir, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("launcher.db");
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE instances(id TEXT PRIMARY KEY);
             INSERT INTO instances VALUES ('one'),('two');
             CREATE TABLE world_map_annotations (
               id INTEGER PRIMARY KEY, instance TEXT NOT NULL, world TEXT NOT NULL,
               dimension TEXT NOT NULL, kind TEXT NOT NULL, name TEXT NOT NULL,
               color INTEGER NOT NULL, points TEXT NOT NULL,
               linked_source TEXT, linked_raw_id TEXT);",
        )
        .unwrap();
    (root, path)
}

fn marker() -> Annotation {
    Annotation {
        id: 0,
        world: WorldId::Save {
            instance: "one".into(),
            folder: "World".into(),
        },
        dimension: Dimension::Overworld,
        kind: AnnotationKind::Marker,
        name: "Home".into(),
        color: [12, 34, 56],
        points: vec![MapPoint { x: -12., z: 8. }],
        linked_source: Some("lumilio.world-explorer".into()),
        linked_raw_id: Some("structure:village:-12:8".into()),
    }
}

#[test]
fn marker_survives_reload_and_cannot_cross_instances() {
    let (_root, path) = database();
    let mut item = marker();
    item.id = put(&path, "one", &item).unwrap();
    assert_eq!(
        list(&path, "one", &item.world, &item.dimension).unwrap(),
        [item.clone()]
    );
    assert!(put(&path, "two", &item).is_err());
    assert!(!remove(&path, "two", item.id).unwrap());
    item.name = "Village".into();
    put(&path, "one", &item).unwrap();
    assert_eq!(
        list(&path, "one", &item.world, &item.dimension).unwrap()[0].name,
        "Village"
    );
    assert!(
        list(&path, "one", &item.world, &Dimension::Nether)
            .unwrap()
            .is_empty()
    );
    assert!(remove(&path, "one", item.id).unwrap());
    assert!(
        list(&path, "one", &item.world, &item.dimension)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn route_length_and_negative_portal_floor() {
    let (_root, path) = database();
    let mut route = marker();
    route.kind = AnnotationKind::Route;
    route.points.push(MapPoint { x: -9., z: 12. });
    assert_eq!(route_length(&route.points), 5.);
    let id = put(&path, "one", &route).unwrap();
    assert_eq!(
        list(&path, "one", &route.world, &route.dimension).unwrap()[0].id,
        id
    );
    assert_eq!(
        portal_coordinates(MapPoint { x: -9., z: 12. }, &Dimension::Overworld),
        Some((-2, 1))
    );
    assert_eq!(
        portal_coordinates(MapPoint { x: -2., z: 1. }, &Dimension::Nether),
        Some((-16, 8))
    );
    assert_eq!(portal_coordinates(route.points[0], &Dimension::End), None);
}
