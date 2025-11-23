# Core Vision (Web, macroquad + WASM)

Origin: adapts the original `20251116_main_idea.md` to a browser-first build where Rust drives everything and macroquad renders via WASM.

## Game premise
- Top-down, tile-based automation + tower defense set on a 3D grid (view one Z-level at a time).
- Automated drones receive plain-text instructions that turn into programs; they mine stone and iron.
- Player spends iron to build defenses (stationary melee warriors) against waves or fortress duels.

## Platform and stack
- Rust crates split into `droneforge-core` (logic) and `droneforge-web` (macroquad render loop).
- Build target: `wasm32-unknown-unknown`; run in-browser via wasm-bindgen glue.
- JS is minimal loader only; all gameplay logic lives in Rust/WASM.

## Camera and layers
- Single active Z-level visible; keyboard `<`/`>` or PageUp/PageDown to change levels.
- 2D camera with pan/zoom; rendering uses colored quads for tiles and simple shapes for units.

## Text → program loop
- Player writes natural language tasks.
- LLM (or stub) outputs JSON AST.
- Rust interpreter executes AST, updating world state and drones.

