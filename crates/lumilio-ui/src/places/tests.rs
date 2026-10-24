use lumilio_core::{GameMode, WorldInfo};

use super::{world_state, world_tags};
use crate::kit::{TagKind, Tone};

#[test]
fn a_world_row_names_its_state_and_tags_in_their_own_forms() {
    let mut save = WorldInfo {
        folder: "w".into(),
        name: "W".into(),
        last_played_ms: Some(1),
        game_version: None,
        hardcore: false,
        game_mode: None,
        cheats: false,
        icon: None,
        damaged: false,
        lock_touched_ms: None,
    };
    assert_eq!(world_state(&save, false), None);
    assert_eq!(world_state(&save, true), Some(("正在游玩", Tone::Lit)));
    assert!(world_tags(&save).is_empty(), "nothing known, nothing shown");

    save.game_mode = Some(GameMode::Survival);
    save.hardcore = true;
    save.cheats = true;
    save.game_version = Some("1.21.1".into());
    assert_eq!(
        world_tags(&save),
        [
            ("极限".to_owned(), TagKind::Ink),
            ("作弊".to_owned(), TagKind::Plain),
            ("1.21.1".to_owned(), TagKind::Plain),
        ]
    );

    // An unreadable world says so, even while a game has it open.
    save.damaged = true;
    assert_eq!(world_state(&save, true), Some(("无法读取", Tone::Failure)));
}
