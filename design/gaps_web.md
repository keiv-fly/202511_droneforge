# Gaps and Next Steps (Web target)

Origin: covers items from `20251116_Missing.md` in the context of the macroquad/WASM build.

## Still missing (to deliver post-plumbing)
- UI scaffolding inside macroquad: HUD, right panel lists, console input, tool strip.
- Player actions: area selection → NL prompt → JSON AST → task execution; warrior placement flow.

## Sequence to close gaps
1. Land the plumbing (black canvas via macroquad WASM) per `stack_setup.md`.
2. Add minimal HUD text rendering and input handling in macroquad.
3. Implement area selection overlays and NL prompt wiring to the AST interpreter.
4. Hook warrior placement to iron cost and WarriorSpot tiles.
5. Add tests for AST validation and deterministic task execution in `droneforge-core`.

