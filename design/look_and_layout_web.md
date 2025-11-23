# Look & Layout (Web, macroquad)

Origin: adapts `20251116_look.md` for the macroquad/WASM target.

## Visual posture
- Top-down, tile-based 2.5D; render one Z-level at a time with colored quads or tiny sprites.
- Keep assets minimal for fast WASM loads; favor flat colors and subtle outlines.
- Drones, enemies, warriors as small shapes; optional flicker/outline for activity.

## Main screen regions
- Center: current Z-level tilemap; selection overlays for tiles/areas/entities.
- Top HUD: Stone, Iron, Wave timer (`Wave N in mm:ss` or `Wave N active`), Core HP, Z-level readout, Pause.
- Right panel: Drones list and Tasks list; clicking focuses/highlights; scrollable.
- Bottom console: NL command entry + log stream; hosts the text→DSL interaction.
- On-map tool strip (top-left of map): Select, Mine Area, Build Warrior, Cancel.

## Controls (mouse-first, keyboard mirrors)
- Mouse: left click select/confirm, left drag area select, right click/ESC cancel, MMB drag or WASD pan, wheel zoom.
- Keyboard: M = Mine mode, W = Build Warrior, `<`/`>` or PageUp/PageDown = Z-level, Space = pause, Enter = focus console, ESC = cancel.

## In-world feedback
- Hover shows tile coords/type; selection outlines; drag rectangles semi-transparent.
- Drone status pips (Idle/Thinking/Working/Finished); task progress text/percent in lists.
- Toasts/log lines for errors (e.g., insufficient iron for warrior placement).

## Macroquad considerations
- Keep UI inside macroquad for now (text overlays/panels) to avoid JS UI libs.
- Fit canvas to viewport; all HUD/panels rendered via macroquad text/shapes for WASM portability.
