# Droneforge Web MVP

Plumbing to render a black canvas from Rust/macroquad compiled to WASM, with a tiny `droneforge-core` world ticking each frame.

## Workspace layout
- `droneforge-core/`: core logic lib (`World` counter)
- `droneforge-web/`: macroquad front-end, built as `cdylib` for wasm
- `web/`: static assets (`index.html`, `main.js`, `style.css`, wasm output later under `web/wasm/`)

## Verify locally
1) Desktop sanity check (quick):  
`cargo run -p droneforge-web`

2) Build WASM target:  
`cargo build -p droneforge-web --release --target wasm32-unknown-unknown`

3) Generate JS glue with wasm-bindgen:  
`wasm-bindgen --target web --no-typescript --out-dir web/wasm --out-name droneforge target/wasm32-unknown-unknown/release/droneforge-web.wasm`

4) Serve the `web/` folder and open in browser:  
`cd web && simple-http-server .`  
Then open the printed URL (e.g., `http://127.0.0.1:8000/`) to see the black canvas (and tick text).

## Prereqs
- `rustup target add wasm32-unknown-unknown`
- Install a static server: `cargo install simple-http-server` (or `miniserve`)
- Optional: `cargo install wasm-bindgen-cli`
