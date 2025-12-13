#![cfg(feature = "generator")]

use std::error::Error;
use std::path::PathBuf;

use d_gen_tileset::generator::build_tileset_image;
mod sprite_atlas;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root();

    let atlas = build_tileset_image();
    let tile_outputs = [
        root.join("assets").join("tileset.png"),
        root.join("web").join("assets").join("tileset.png"),
    ];

    for path in tile_outputs {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        atlas.save(&path)?;
        println!("wrote {}", path.display());
    }

    let sprites = sprite_atlas::build_drone_sprite_atlas(&root)?;
    let sprite_outputs = [
        root.join("assets").join("sprites.png"),
        root.join("web").join("assets").join("sprites.png"),
    ];

    for path in sprite_outputs {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        sprites.save(&path)?;
        println!("wrote {}", path.display());
    }

    Ok(())
}
