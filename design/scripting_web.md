# Scripting Architecture (Web, JSON AST → Rust in WASM)

Origin: adapts `20251116_scripting.md` to the browser target while keeping the JSON AST as the single source of truth.

## Three layers
1. Surface NL (player input) — untrusted.
2. Canonical JSON AST — versioned schema; validated and executed in Rust inside WASM.
3. Derived views — optional pretty-printed DSL and lowered IR/bytecode from the AST.

## Minimal schema (unchanged core, web-safe)
- Top-level: `{ "version": 1, "node": "Program", "statements": [...] }`
- Nodes: `Let`, `TileBoxFromCoords`, `TileCoord3`/`TileBox3`, `ExprStmt`, `Call`, optional `ForIn` + `IterTiles`.
- Host calls: `mine_box(box)`, optionally `build_wall_on_border(box)`, later expansion.

## Type model
- Primitive: `Int`, `Bool`.
- Spatial: `TileCoord3 { x, y, z }`, `TileBox3 { min, max }` with invariants `min <= max`.
- First-class in the AST; mirror Rust structs for zero-copy interop.

## Execution in Rust (WASM)
- Validate JSON → name resolution → type-check → execute.
- Host function registry in Rust; each call mapped to game logic (mining, building).
- Randomness stays host-side (e.g., `rand_int(min,max)`), seeded per world/drone for determinism.

## Example program
```json
{
  "version": 1,
  "node": "Program",
  "statements": [
    {
      "node": "Let",
      "name": "area",
      "ty": "TileBox3",
      "value": {
        "node": "TileBoxFromCoords",
        "min": { "node": "TileCoord3", "x": 10, "y": 5, "z": 0 },
        "max": { "node": "TileCoord3", "x": 20, "y": 7, "z": 0 }
      }
    },
    {
      "node": "ExprStmt",
      "expr": {
        "node": "Call",
        "func": "mine_box",
        "args": [
          { "node": "VarRef", "name": "area" }
        ]
      }
    }
  ]
}
```

## WASM-friendly guidance
- Keep serde/serde_json for parsing; no JS parsing of gameplay logic.
- JS only loads the WASM and calls exported Rust entrypoints; all AST validation/execution stays in Rust.
- Avoid non-deterministic JS hooks; keep determinism inside Rust for reproducible tests.
