# Droneforge Web MVP

Plumbing to render a black canvas from Rust/macroquad compiled to WASM, with a tiny `droneforge-core` world ticking each frame.

## Workspace layout
- `droneforge-core/`: core logic lib (`World` counter)
- `droneforge-web/`: macroquad front-end, built as `cdylib` for wasm
- `web/`: static assets (`index.html`, `main.js`, `style.css`, wasm output later under `web/wasm/`)

## Verify locally
1) Desktop sanity check (quick):  
`cargo run -p droneforge-web`

2) Build WASM target (macroquad emits both a lib & bin artifact, we want the bin):  
`cargo build -p droneforge-web --release --target wasm32-unknown-unknown`

3) Copy the produced module next to the HTML so `mq_js_bundle.js` can load it:  
`cp target/wasm32-unknown-unknown/release/droneforge-web.wasm web/droneforge-web.wasm`

4) Serve the `web/` folder and open in browser:  
`cd web; simple-http-server .`  
Then open the printed URL (e.g., `http://127.0.0.1:8000/`) to see the black canvas (and tick text).  
(`web/index.html` already references the vendored `mq_js_bundle.js` from macroquad’s docs.)

## Prereqs
- `rustup target add wasm32-unknown-unknown`
- Install a static server: `cargo install simple-http-server` (or `miniserve`)
