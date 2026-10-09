use super::*;

fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    }
}

#[test]
fn an_explicit_home_wins() {
    let root = data_root(env(&[("LUMILIO_HOME", "/data/lumilio"), ("HOME", "/h")]));
    assert_eq!(root, Some(PathBuf::from("/data/lumilio")));
}

#[test]
fn an_empty_override_is_ignored() {
    let root = data_root(env(&[
        ("LUMILIO_HOME", ""),
        ("HOME", "/h"),
        ("APPDATA", "/a"),
    ]));
    assert!(root.is_some_and(|path| path.ends_with("LumilioCL") || path.ends_with("lumilio")));
}

#[test]
fn nothing_to_go_on_means_no_root() {
    assert_eq!(data_root(env(&[])), None);
}

#[test]
fn the_backend_opens_and_runs_work_off_the_calling_thread() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::open(dir.path().join("root")).unwrap();
    let service = backend.service.clone();
    let handle = backend.spawn(async move { service.library().await.instances.len() });
    let count = futures_block(handle);
    assert_eq!(count, 0);
}

#[test]
fn world_map_is_available_without_saves_and_manual_seed_survives_restart() {
    assert_eq!(
        lumilio_ui::world_explorer::MANUAL_VERSIONS,
        lumilio_plugin_world_explorer::SUPPORTED_VERSIONS
    );
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let backend = Backend::open(dir.path().join("root")).unwrap();
        let service = backend.service.clone();
        futures_block(backend.spawn(async move {
            let record = service
                .create_instance("Seed", Some("1.21.4"), lumilio_core::Loader::Vanilla, None)
                .await
                .unwrap();
            assert!(
                service
                    .plugin_tabs(&record.id)
                    .await
                    .unwrap()
                    .iter()
                    .any(|tab| tab.plugin == lumilio_plugin_world_explorer::ID)
            );
            assert_eq!(
                service
                    .plugin_view(&record.id, lumilio_plugin_world_explorer::ID)
                    .await
                    .unwrap(),
                Some(lumilio_plugin_api::View::Map)
            );
            service
                .save_map_seed(&record.id, 262, "1.21.4")
                .await
                .unwrap();
            assert_eq!(
                service.map_contexts(&record.id).await.unwrap()[0]
                    .context
                    .seed,
                Some(262)
            );
            // The whole overlay path through the shipped host: the catalog for
            // this world, then objects for one of its layers.
            let context = service.map_contexts(&record.id).await.unwrap()[0]
                .context
                .clone();
            let layers = service.map_overlays(&context).await;
            let village = layers
                .iter()
                .find(|(_, layer)| layer.id == "structure.village")
                .expect("villages are offered for a 1.21.4 seed");
            let found = service
                .map_objects(
                    &record.id,
                    &village.0,
                    lumilio_plugin_api::map::OverlayRequest {
                        context,
                        overlay: "structure.village".into(),
                        bounds: lumilio_plugin_api::map::MapBounds {
                            min: lumilio_plugin_api::map::MapPoint {
                                x: -1536.,
                                z: -1536.,
                            },
                            max: lumilio_plugin_api::map::MapPoint { x: 1536., z: 1536. },
                        },
                        level: 0,
                    },
                    lumilio_core::CancellationToken::new(),
                )
                .await;
            assert_eq!(found.map(|objects| objects.len()), Ok(7));
            record.id
        }))
    };
    let backend = Backend::open(dir.path().join("root")).unwrap();
    let service = backend.service.clone();
    futures_block(backend.spawn(async move {
        assert_eq!(
            service.map_contexts(&id).await.unwrap()[0].context.seed,
            Some(262)
        );
        service
            .set_plugin_enabled(lumilio_plugin_world_explorer::ID, false)
            .await
            .unwrap();
        assert!(
            !service
                .plugin_tabs(&id)
                .await
                .unwrap()
                .iter()
                .any(|tab| tab.plugin == lumilio_plugin_world_explorer::ID)
        );
        assert!(
            !service
                .map_providers()
                .await
                .base_maps
                .iter()
                .any(|(plugin, _)| plugin == lumilio_plugin_world_explorer::ID)
        );
    }));
}

#[test]
fn the_shipped_backend_registers_the_crash_analyzer_and_its_switch() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::open(dir.path().join("root")).unwrap();
    let service = backend.service.clone();
    futures_block(backend.spawn(async move {
        let plugins = service.plugins().await;
        assert_eq!(plugins.len(), 5);
        let id = &plugins[0].manifest.id;
        assert_eq!(id, lumilio_plugin_crash_analyzer::ID);
        assert_eq!(plugins[1].manifest.id, lumilio_plugin_discord::ID);
        assert_eq!(plugins[1].status, lumilio_core::PluginStatus::Disabled);
        assert_eq!(plugins[2].manifest.id, lumilio_plugin_litematica::ID);
        assert_eq!(plugins[3].manifest.id, lumilio_plugin_modrinth::ID);
        assert_eq!(plugins[4].manifest.id, lumilio_plugin_world_explorer::ID);
        assert_eq!(plugins[4].status, lumilio_core::PluginStatus::Enabled);
        // Discover has its source from the first start, with no setup.
        assert_eq!(
            plugins[3].status,
            lumilio_core::PluginStatus::Enabled,
            "Modrinth is on by default"
        );
        let record = service
            .create_instance("Report", Some("1.0"), lumilio_core::Loader::Vanilla, None)
            .await
            .unwrap();
        let reports = service.layout().game(&record.id).join("crash-reports");
        tokio::fs::create_dir_all(&reports).await.unwrap();
        tokio::fs::write(
            reports.join("crash.txt"),
            "java.lang.OutOfMemoryError: heap",
        )
        .await
        .unwrap();
        let (text, findings) = service.crash_report(&record.id, "crash.txt").await.unwrap();
        assert!(text.contains("OutOfMemoryError"));
        assert_eq!(findings[0].finding.rule, "out-of-memory");
        service.set_plugin_enabled(id, false).await.unwrap();
        assert!(
            service
                .crash_report(&record.id, "crash.txt")
                .await
                .unwrap()
                .1
                .is_empty()
        );
        service.set_plugin_enabled(id, true).await.unwrap();
        assert_eq!(
            service
                .crash_report(&record.id, "crash.txt")
                .await
                .unwrap()
                .1
                .len(),
            1
        );
    }));
}

/// Polls a join handle without a runtime of our own, the way GPUI awaits it.
fn futures_block<T>(handle: tokio::task::JoinHandle<T>) -> T {
    use std::sync::mpsc;
    use std::task::{Context, Poll, Wake, Waker};
    struct Ping(mpsc::Sender<()>);
    impl Wake for Ping {
        fn wake(self: Arc<Self>) {
            let _ = self.0.send(());
        }
    }
    let (tx, rx) = mpsc::channel();
    let waker = Waker::from(Arc::new(Ping(tx)));
    let mut cx = Context::from_waker(&waker);
    let mut handle = Box::pin(handle);
    loop {
        if let Poll::Ready(value) = handle.as_mut().poll(&mut cx) {
            return value.unwrap();
        }
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    }
}

#[test]
fn the_litematica_tab_follows_the_schematics_folder_and_its_switch() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::open(dir.path().join("root")).unwrap();
    let service = backend.service.clone();
    futures_block(backend.spawn(async move {
        let record = service
            .create_instance("Builder", Some("1.0"), lumilio_core::Loader::Vanilla, None)
            .await
            .unwrap();
        assert_eq!(
            service.plugin_tabs(&record.id).await.unwrap()[0].plugin,
            lumilio_plugin_world_explorer::ID
        );

        let folder = service.layout().game(&record.id).join("schematics");
        tokio::fs::create_dir_all(&folder).await.unwrap();
        tokio::fs::write(folder.join("broken.litematic"), b"not a schematic")
            .await
            .unwrap();
        let tabs = service.plugin_tabs(&record.id).await.unwrap();
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs[0].plugin, lumilio_plugin_litematica::ID);

        // A broken file is one row that says so; the list still shows.
        let view = service
            .plugin_view(&record.id, lumilio_plugin_litematica::ID)
            .await
            .unwrap();
        let Some(lumilio_plugin_api::View::List { items }) = view else {
            panic!("expected a list");
        };
        assert_eq!(items[0].subtitle.as_deref(), Some("读不了"));

        // The detail offers a 3D preview of that file, and the host serves it.
        // The host returns raw bytes; the native renderer handles parse errors.
        assert_eq!(
            service
                .plugin_model(
                    &record.id,
                    lumilio_plugin_litematica::ID,
                    "schematics/broken.litematic",
                )
                .await
                .unwrap()
                .schematic,
            b"not a schematic"
        );
        assert_eq!(service.plugin_tabs(&record.id).await.unwrap().len(), 2);
        assert!(
            service
                .plugin_model(&record.id, lumilio_plugin_litematica::ID, "options.txt")
                .await
                .is_err()
        );

        service
            .set_plugin_enabled(lumilio_plugin_litematica::ID, false)
            .await
            .unwrap();
        assert_eq!(
            service.plugin_tabs(&record.id).await.unwrap()[0].plugin,
            lumilio_plugin_world_explorer::ID
        );
        assert!(
            service
                .plugin_view(&record.id, lumilio_plugin_litematica::ID)
                .await
                .unwrap()
                .is_none()
        );
    }));
}
