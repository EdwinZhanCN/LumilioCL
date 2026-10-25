use std::rc::Rc;

use lumilio_core::{FoundGame, GameOrigin, Loader};

use super::super::game_picker::*;

fn game(name: &str, loader: Loader) -> FoundGame {
    FoundGame {
        name: name.into(),
        game_version: "1.21.1".into(),
        loader,
        loader_version: None,
        game_dir: "/x".into(),
        origin: GameOrigin::MultiMcPrism,
    }
}

#[test]
fn the_first_game_starts_ticked_and_the_ticked_ones_are_handed_over_in_order() {
    let picker = GamePicker::new(
        vec![
            game("a", Loader::Vanilla),
            game("b", Loader::Fabric),
            game("c", Loader::Forge),
        ],
        Rc::new(|_, _, _| {}),
    );
    let names = |rows: &[(FoundGame, bool)]| -> Vec<String> {
        chosen(rows).into_iter().map(|g| g.name).collect()
    };
    assert_eq!(names(&picker.rows), ["a"]);
    let mut rows = picker.rows;
    rows[2].1 = true;
    rows[0].1 = false;
    assert_eq!(names(&rows), ["c"]);
    rows[1].1 = true;
    assert_eq!(names(&rows), ["b", "c"]);
    assert_eq!(describe(&rows[1].0), "b · 1.21.1 Fabric · MultiMC / Prism");
}
