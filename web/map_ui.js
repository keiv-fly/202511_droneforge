function forwardZChange(callbackName) {
    const handler = wasm_exports?.[callbackName];
    if (typeof handler === "function") {
        handler();
    }
}

const selectionDecoder = new TextDecoder("utf-8");

function readWasmString(ptr, len) {
    if (!wasm_memory || !ptr || len <= 0) return "";
    try {
        const view = new Uint8Array(wasm_memory.buffer, ptr, len);
        return selectionDecoder.decode(view);
    } catch (error) {
        console.warn("failed to read wasm string", error);
        return "";
    }
}

window.addEventListener("load", () => {
    const zUp = document.getElementById("z-up");
    const zDown = document.getElementById("z-down");
    const canvas = document.getElementById("glcanvas");
    const selectionPanel = document.getElementById("selection-panel");
    const selectionText = document.getElementById("selection-text");
    const selectionMove = document.getElementById("selection-move");
    const selectionUse = document.getElementById("selection-use");

    if (canvas) {
        canvas.addEventListener("contextmenu", (event) => {
            event.preventDefault();
        });
    }

    zUp.addEventListener("click", () => {
        forwardZChange("z_level_up");
    });

    zDown.addEventListener("click", () => {
        forwardZChange("z_level_down");
    });

    const pumpSelectionUi = () => {
        const presentFn = wasm_exports?.selected_drone_present;
        const isPresent = typeof presentFn === "function" && presentFn() === 1;
        if (isPresent) {
            const namePtrFn = wasm_exports?.selected_drone_name_ptr;
            const nameLenFn = wasm_exports?.selected_drone_name_len;
            const healthFn = wasm_exports?.selected_drone_health;
            const healthMaxFn = wasm_exports?.selected_drone_health_max;

            const namePtr = typeof namePtrFn === "function" ? namePtrFn() : 0;
            const nameLen = typeof nameLenFn === "function" ? nameLenFn() : 0;
            const name = nameLen > 0 ? readWasmString(namePtr, nameLen) : "";
            const hp = typeof healthFn === "function" ? healthFn() : 0;
            const hpMax = typeof healthMaxFn === "function" ? healthMaxFn() : 0;

            if (selectionText) {
                const safeName = name || "???";
                const maxDisplay = hpMax > 0 ? hpMax : 0;
                selectionText.textContent = `drone ${safeName} hp ${hp}/${maxDisplay}`;
            }
            if (selectionPanel) {
                selectionPanel.style.display = "flex";
            }
        } else {
            if (selectionPanel) {
                selectionPanel.style.display = "none";
            }
            if (selectionText) {
                selectionText.textContent = "";
            }
        }
        requestAnimationFrame(pumpSelectionUi);
    };

    if (selectionPanel && selectionText) {
        requestAnimationFrame(pumpSelectionUi);
    }

    if (selectionMove) {
        selectionMove.addEventListener("click", () => {
            const fn = wasm_exports?.drone_action_move;
            if (typeof fn === "function") {
                fn();
            }
        });
    }

    if (selectionUse) {
        selectionUse.addEventListener("click", () => {
            const fn = wasm_exports?.drone_action_use;
            if (typeof fn === "function") {
                fn();
            }
        });
    }
});

