use std::path::Path;
use std::time::Duration;

use super::{GAME_OUTPUT_LOG, OutputFile};

fn record(game_dir: &Path) -> String {
    std::fs::read_to_string(game_dir.join("logs").join(GAME_OUTPUT_LOG)).unwrap()
}

#[test]
fn each_launch_replaces_the_record_and_it_says_how_the_game_ended() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = OutputFile::create(dir.path(), &["first launch".to_owned()]).unwrap();
    first.line("[main/INFO]: from before");
    drop(first);

    let mut output = OutputFile::create(dir.path(), &["second launch".to_owned()]).unwrap();
    output.note("a note from the account");
    output.line("Exception in thread \"main\"");
    output.ended(Some(1), Duration::from_millis(400), false);
    assert_eq!(
        record(dir.path()),
        "# second launch\n# a note from the account\nException in thread \"main\"\n\
         # The game ended with exit code 1 after 0.4 s.\n"
    );

    output.ended(None, Duration::from_secs(2), true);
    assert!(record(dir.path()).ends_with("# The launcher stopped the game after 2.0 s.\n"));
}

#[test]
fn past_the_limit_only_the_ending_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let mut output = OutputFile::create(dir.path(), &[]).unwrap();
    output.limit = 12;
    output.line("0123456789");
    output.line("more");
    output.line("and more");
    output.ended(None, Duration::from_secs(2), false);
    assert_eq!(
        record(dir.path()),
        "0123456789\n# The output goes on past 32 MiB; the game's own logs have the rest.\n\
         # The game ended without an exit code after 2.0 s.\n"
    );
}
