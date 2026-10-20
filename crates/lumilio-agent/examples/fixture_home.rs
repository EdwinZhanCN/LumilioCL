//! Makes a launcher data folder with one game and one schematic, for trying
//! `lumiliocl mcp` by hand:
//!
//! ```text
//! cargo run -p lumilio-agent --example fixture_home -- /tmp/lumilio-home
//! LUMILIO_HOME=/tmp/lumilio-home target/debug/lumiliocl mcp
//! ```
//!
//! The folder must not exist yet; nothing of the real library is touched.

use std::path::PathBuf;

use lumilio_core::{InstanceStore, Layout, Loader, NewInstance};

/// A small schematic from Nucleation's samples (MIT).
const SAMPLE: &[u8] = include_bytes!("../../../forks/nucleation/tests/samples/3x3.litematic");

fn main() {
    let Some(home) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: fixture_home <new data folder>");
        std::process::exit(2);
    };
    if home.exists() {
        eprintln!("{} exists already; choose a new folder", home.display());
        std::process::exit(2);
    }
    std::fs::create_dir_all(&home).expect("create the data folder");
    let id = {
        let mut store = InstanceStore::open(&home).expect("open the library");
        let record = store
            .create(
                NewInstance {
                    name: "Redstone lab".into(),
                    game_version: "1.21.4".into(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                },
                1_700_000_000,
            )
            .expect("create the game");
        record.id.clone()
    };
    let folder = Layout::new(&home).game(&id).join("schematics");
    std::fs::create_dir_all(&folder).expect("create the schematics folder");
    std::fs::write(folder.join("3x3.litematic"), SAMPLE).expect("write the schematic");
    println!("{id}");
}
