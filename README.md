# Droneforge Web MVP

Plumbing to render a black canvas from Rust/macroquad compiled to WASM, with a tiny `droneforge-core` world ticking each frame.

## Workspace layout
- `droneforge-core/`: core logic lib (`World` counter)
- `droneforge-web/`: macroquad front-end, built as `cdylib` for wasm
- `d-gen-tileset/`: utility to generate the shared tileset atlas
- `web/`: static assets (`index.html`, `main.js`, `style.css`, wasm output later under `web/`)

## Tileset atlas
- Regenerate with `cargo run -p d-gen-tileset`.
- Outputs `assets/tileset.png` (native) and `web/assets/tileset.png` (served with WASM).
- Re-run after changing tile colors/layout so the game loads the refreshed atlas at runtime.
- The PowerShell helper `run.ps1` builds the WASM, copies it into `web/`, and serves the folder on `http://127.0.0.1:8000/` (opens a browser; stop the server with its printed PID).

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
(`web/index.html` already references the vendored `mq_js_bundle.js` from macroquad’s web.)

5) Regenerate the tileset atlas after tile changes:  
`cargo run -p d-gen-tileset`

## Prereqs
- `rustup target add wasm32-unknown-unknown`
- Install a static server: `cargo install simple-http-server` (or `miniserve`)

## Hosted on GitHub Pages

The game can be checked here:
https://keiv-fly.github.io/202511_droneforge/

## Load-time benchmark
- Requires Node 16+; run `cd benchmark && npm install` once (downloads Playwright).
- Serve the game (e.g., `cd web && simple-http-server .`) or point to the hosted page. On npm 10 + PowerShell, forward args with a double `--`:  
  `cd benchmark && npm run benchmark -- -- --url http://127.0.0.1:8005/ --runs=10`  
  (Direct call also works: `node run.js --url http://127.0.0.1:8005/ --runs=10`).
- The benchmark script will, by default, build the WASM (`cargo build -p droneforge-web --release --target wasm32-unknown-unknown`), copy it into `web/`, kill any process listening on port 8005, and start `simple-http-server` on that port before running. Set `--skip-prepare` or `DF_BENCH_SKIP_PREPARE=1` to reuse an existing server.
- The runner writes `benchmark/results/load-benchmark.json` and `benchmark/results/load-benchmark.md` with per-run and aggregate metrics (OS, browser, CPU included). Include the refreshed files with every PR.
- For quick structure checks without launching a browser, run `DF_BENCH_MOCK=1 node benchmark/run.js -- --mock` (or add `--runs=N` as needed).