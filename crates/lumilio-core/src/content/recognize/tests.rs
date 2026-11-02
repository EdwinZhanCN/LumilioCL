//! What the sources said is kept between lists (plan content-recognition D2).

use crate::content::discover::ProjectKind;
use crate::content::{ContentList, ContentLookup};
use crate::testing::*;

const IDENTIFY: &str = "https://api.modrinth.com/v2/version_files";
const UPDATES: &str = "https://api.modrinth.com/v2/version_files/update";
const PROJECTS: &str = "https://api.modrinth.com/v2/projects?";

fn modrinth_version(project: &str, name: &str, body: &[u8]) -> String {
    format!(
        r#"{{"id":"v-{project}","project_id":"{project}","name":"{name}","version_number":"1",
            "version_type":"release","game_versions":["1.21.1"],"loaders":["fabric"],
            "date_published":"2026-01-01T00:00:00Z",
            "files":[{{"url":"https://cdn.modrinth.com/{name}","filename":"{name}","primary":true,
                       "size":{},"hashes":{{"sha1":"{}"}}}}],"dependencies":[]}}"#,
        body.len(),
        sha1_hex(body)
    )
}

/// Modrinth knows `known` as project `MR`, named "Kept Mod".
fn answer_modrinth(world: &World, known: &[u8]) {
    world
        .net
        .answer("api.modrinth.com/v2/version_files/update", "{}");
    world.net.answer(
        "api.modrinth.com/v2/version_files",
        format!(
            r#"{{"{}":{}}}"#,
            sha1_hex(known),
            modrinth_version("MR", "a.jar", known)
        ),
    );
    world.net.answer(
        "api.modrinth.com/v2/projects",
        r#"[{"id":"MR","slug":"kept","title":"Kept Mod","project_type":"mod"}]"#,
    );
    world.net.answer("api.modrinth.com/v2/teams", "[]");
}

fn asked(world: &World) -> (usize, usize, usize) {
    let projects = world
        .net
        .sent
        .lock()
        .unwrap()
        .iter()
        .filter(|request| request.url.starts_with(PROJECTS))
        .count();
    (
        world.net.sent_to(IDENTIFY),
        world.net.sent_to(UPDATES),
        projects,
    )
}

fn title<'a>(list: &'a ContentList, file: &str) -> Option<&'a str> {
    list.entries
        .iter()
        .find(|entry| entry.item.file_name == file)?
        .source
        .as_ref()
        .map(|source| source.title.as_str())
}

#[tokio::test]
async fn a_second_list_takes_what_the_source_said_without_asking_again() {
    let world = world();
    let record = fabric_instance(&world, "1.21.1").await;
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("a.jar"), b"modrinth jar").unwrap();
    answer_modrinth(&world, b"modrinth jar");
    let details = |lookup| {
        world
            .service
            .content_details(&record.id, ProjectKind::Mod, lookup)
    };

    let first = details(ContentLookup::Fresh).await.unwrap();
    assert_eq!(title(&first, "a.jar"), Some("Kept Mod"));
    // The update check identifies the files itself and may ask per channel.
    let once = asked(&world);
    assert!(once.0 >= 1 && once.1 >= 1 && once.2 == 1, "{once:?}");
    let again = details(ContentLookup::Fresh).await.unwrap();
    assert_eq!(again.entries, first.entries);
    assert_eq!(asked(&world), once);

    // A new file is asked about alone; the known one is not asked again,
    // nor is it after it is switched off (its name changed, not its content).
    std::fs::write(mods.join("b.jar"), b"own jar").unwrap();
    crate::content::set_enabled(
        &world.service.layout.game(&record.id),
        ProjectKind::Mod,
        "a.jar",
        false,
    )
    .unwrap();
    let both = details(ContentLookup::Fresh).await.unwrap();
    assert_eq!(title(&both, "a.jar.disabled"), Some("Kept Mod"));
    assert_eq!(title(&both, "b.jar"), None);
    assert_eq!(asked(&world), (once.0 + 1, once.1, once.2));
    let body = world
        .net
        .sent
        .lock()
        .unwrap()
        .iter()
        .rfind(|request| request.url == IDENTIFY)
        .and_then(|request| request.body.clone())
        .unwrap();
    let body = String::from_utf8(body).unwrap();
    assert!(body.contains(&sha1_hex(b"own jar")), "{body}");
    assert!(!body.contains(&sha1_hex(b"modrinth jar")), "{body}");
    // That the new file is unknown is kept too.
    details(ContentLookup::Fresh).await.unwrap();
    assert_eq!(asked(&world), (once.0 + 1, once.1, once.2));
}

#[tokio::test]
async fn what_is_kept_answers_without_the_network_and_outlasts_a_failed_ask() {
    let world = world();
    let record = fabric_instance(&world, "1.21.1").await;
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("a.jar"), b"modrinth jar").unwrap();
    let details = |lookup| {
        world
            .service
            .content_details(&record.id, ProjectKind::Mod, lookup)
    };

    // Nothing kept yet: the file is listed without a source, nothing asked.
    let kept = details(ContentLookup::Kept).await.unwrap();
    assert_eq!(title(&kept, "a.jar"), None);
    assert!(!kept.sources_unavailable);
    assert_eq!(asked(&world), (0, 0, 0));

    answer_modrinth(&world, b"modrinth jar");
    details(ContentLookup::Fresh).await.unwrap();
    let once = asked(&world);
    world.net.forget_all();
    let kept = details(ContentLookup::Kept).await.unwrap();
    assert_eq!(title(&kept, "a.jar"), Some("Kept Mod"));
    assert!(!kept.sources_unavailable);
    assert_eq!(asked(&world), once);

    // Asked again with Modrinth out of reach, the file keeps its source and
    // the list says the source could not be asked.
    let again = details(ContentLookup::Again).await.unwrap();
    assert!(world.net.sent_to(IDENTIFY) > once.0);
    assert_eq!(title(&again, "a.jar"), Some("Kept Mod"));
    assert!(again.sources_unavailable);
}
