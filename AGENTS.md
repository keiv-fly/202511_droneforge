# Repository Guidelines

## Project Structure & Module Organization
- Workspace root `Cargo.toml` ties together `droneforge-core/` (world simulation library) and `droneforge-web/` (Macroquad front-end built for native and WASM).  
- `docs/` hosts the static site for GitHub Pages, including `droneforge-web.wasm`, `index.html`, and the vendored `mq_js_bundle.js`.  
- `design/` contains reference notes and flows; `target/` is build output and can be cleaned freely.

## Build, Test, and Development Commands
- Format and lint before commits: `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`.  
- Run all tests: `cargo test --workspace`. Add focused tests next to the code they cover.  
- Desktop sanity run: `cargo run -p droneforge-web` (opens the Macroquad window).  
- WASM build: `cargo build -p droneforge-web --release --target wasm32-unknown-unknown`; copy the output with `cp target/wasm32-unknown-unknown/release/droneforge-web.wasm docs/`.  
- Serve the web build locally: `cd docs && simple-http-server .` (or `miniserve .`) and open the printed URL.
- Before finishing any agent task, re-run the README flow: build `droneforge-web` for `wasm32-unknown-unknown` and copy the resulting `droneforge-web.wasm` into `docs/` so GitHub Pages stays current.

## Coding Style & Naming Conventions
- Rust 2024, 4-space indentation, `snake_case` for functions/variables, `CamelCase` for types. Keep modules small and favor explicit `pub` exposure.  
- Prefer immutable bindings; keep `GameState`-style structs managing state, with pure helpers where possible.  
- Use `cargo fmt` output as the source of truth; align Macroquad code with clear frame/update separation (`fixed_update`, `render`).

## Testing Guidelines
- Use `cargo test --workspace` as the baseline gate; for new logic in `droneforge-core`, add `#[cfg(test)]` unit tests alongside the implementation.  
- For rendering or timing changes, document a manual smoke check (native run and `docs/` serve) in the PR description.  
- Aim to keep builds warning-free under `cargo clippy -- -D warnings`.

## Commit & Pull Request Guidelines
- Follow the existing history: concise, present-tense subjects (e.g., `Add link to hosted game`, `Separate engine steps`), ideally ≤72 characters.  
- Keep a focused commit per concern; avoid bundling generated `droneforge-web.wasm` unless the behavior changed and the binary was rebuilt.  
- PRs should describe scope, testing performed (commands run), and any visual/output changes; link issues when applicable and include screenshots/GIFs for noticeable UI differences.  
- Before opening a PR, run fmt, clippy, tests, and (for web changes) rebuild and copy the WASM into `docs/` so the hosted page stays in sync.
