use super::super::error::ServiceError;
use super::world;
use crate::instance::Loader;
use crate::servers::{PackPolicy, ServerEntry, ServerError};

fn entry(name: &str, address: &str) -> ServerEntry {
    ServerEntry {
        name: name.into(),
        address: address.into(),
        packs: PackPolicy::Ask,
    }
}

#[tokio::test]
async fn servers_are_edited_under_the_lease_and_a_stale_row_is_refused() {
    let world = world();
    let record = world
        .service
        .create_instance("Servers", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let id = &record.id;
    assert!(world.service.servers(id).await.unwrap().is_empty());

    world
        .service
        .add_server(id, entry("A", "a.example"))
        .await
        .unwrap();
    world
        .service
        .add_server(id, entry("B", "b.example"))
        .await
        .unwrap();
    {
        let _busy = world.service.reserve_instance(id).unwrap();
        assert!(matches!(
            world.service.add_server(id, entry("C", "c")).await,
            Err(ServiceError::InstanceBusy(_))
        ));
        assert_eq!(world.service.servers(id).await.unwrap().len(), 2);
    }

    world
        .service
        .move_server(id, 1, entry("B", "b.example"), 0)
        .await
        .unwrap();
    let names: Vec<_> = world
        .service
        .servers(id)
        .await
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, ["B", "A"]);
    assert!(matches!(
        world
            .service
            .remove_server(id, 0, entry("A", "a.example"))
            .await,
        Err(ServiceError::Server(ServerError::Changed))
    ));
    world
        .service
        .update_server(id, 1, entry("A", "a.example"), entry("A2", "a2.example:1"))
        .await
        .unwrap();
    world
        .service
        .remove_server(id, 0, entry("B", "b.example"))
        .await
        .unwrap();
    assert_eq!(
        world.service.servers(id).await.unwrap(),
        [entry("A2", "a2.example:1")]
    );
}
