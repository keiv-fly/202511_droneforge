# UI Flows (Web macroquad)

Origin: adapts `20251116_UI.md` to the macroquad/WASM stack and mouse-first flows.

## Layout summary
- Top HUD: Stone, Iron, Wave timer, Core HP, Z-level controls, Pause.
- Center map: current Z-level; pan/zoom; hover tooltip with coords/type; selection overlays.
- Right panel: Drones and Tasks lists; click to focus camera/highlight areas.
- Bottom console: NL task input + log stream.
- Tool strip (map overlay, top-left): Select, Mine Area, Build Warrior, Cancel.

## Modes and flows
- Select: left click to inspect; right panel reflects selection; right click/ESC to clear.
- Mine Area (M or tool strip): drag rectangle → console auto-focuses prompt → submit NL → JSON AST → task assigned to idle drone; statuses: Thinking → Working → Finished.
- Build Warrior (W or tool strip): hover WarriorSpot shows cost; left click places if `iron >= cost`; else toast/log error; right click/ESC cancels.
- Cancel: ESC or right click exits current mode/drag.

## Controls
- Mouse: left click select/confirm; left drag area; right click cancel; MMB drag or WASD pan; wheel zoom.
- Keyboard: M (mine), W (build warrior), `<`/`>` or PageUp/PageDown (Z-level), Space (pause), Enter (focus console), ESC (cancel).

## Feedback cues
- Drag overlay with translucent fill and border; selected entity outlines; drone/task highlights on list click.
- Drone status pips; task progress text; wave/core/resource HUD values always visible.
- Errors/warnings: short toasts near HUD plus persistent console log lines.

## Macroquad/WASM notes
- Render HUD and panels via macroquad text/shapes for WASM compatibility.
- Keep canvas full-viewport; avoid external JS UI libs; JS only loads WASM/starts the loop.
