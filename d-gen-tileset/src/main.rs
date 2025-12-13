#![cfg(feature = "generator")]

use std::error::Error;
use std::path::PathBuf;

use d_gen_tileset::generator::build_tileset_image;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn main() -> Result<(), Box<dyn Error>> {
    let atlas = build_tileset_image();
    let root = workspace_root();
    let outputs = [
        root.join("assets").join("tileset.png"),
        root.join("web").join("assets").join("tileset.png"),
    ];

    for path in outputs {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atlas.save(&path)?;
        println!("wrote {}", path.display());
    }

    Ok(())
}
