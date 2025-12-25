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
    const selectionProgress = document.getElementById("selection-progress");
    const selectionProgressTrack = document.getElementById(
        "selection-progress-track"
    );
    const selectionProgressFill = document.getElementById(
        "selection-progress-fill"
    );

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
            const statusPtrFn = wasm_exports?.selected_drone_status_ptr;
            const statusLenFn = wasm_exports?.selected_drone_status_len;

            const namePtr = typeof namePtrFn === "function" ? namePtrFn() : 0;
            const nameLen = typeof nameLenFn === "function" ? nameLenFn() : 0;
            const name = nameLen > 0 ? readWasmString(namePtr, nameLen) : "";
            const hp = typeof healthFn === "function" ? healthFn() : 0;
            const hpMax = typeof healthMaxFn === "function" ? healthMaxFn() : 0;
            const statusPtr = typeof statusPtrFn === "function" ? statusPtrFn() : 0;
            const statusLen = typeof statusLenFn === "function" ? statusLenFn() : 0;
            const status =
                statusLen > 0 ? readWasmString(statusPtr, statusLen) : "";

            if (selectionText) {
                const safeName = name || "???";
                const maxDisplay = hpMax > 0 ? hpMax : 0;
                const headerText = `drone ${safeName} hp ${hp}/${maxDisplay}`;
                selectionText.textContent = status
                    ? `${headerText}\n${status}`
                    : headerText;
            }
            if (selectionPanel) {
                selectionPanel.style.display = "flex";
            }

            const progressVisibleFn = wasm_exports?.selected_drone_progress_visible;
            const progressPercentFn = wasm_exports?.selected_drone_progress_percent;
            const showProgress =
                typeof progressVisibleFn === "function" &&
                progressVisibleFn() === 1;

            if (selectionProgress && selectionProgressFill) {
                if (showProgress) {
                    const percent =
                        typeof progressPercentFn === "function"
                            ? progressPercentFn()
                            : 0;
                    selectionProgress.style.display = "block";
                    selectionProgressFill.style.width = `${percent}%`;
                    if (selectionProgressTrack) {
                        selectionProgressTrack.setAttribute(
                            "aria-valuenow",
                            `${percent}`
                        );
                    }
                } else {
                    selectionProgress.style.display = "none";
                    selectionProgressFill.style.width = "0%";
                    if (selectionProgressTrack) {
                        selectionProgressTrack.setAttribute("aria-valuenow", "0");
                    }
                }
            }
        } else {
            if (selectionPanel) {
                selectionPanel.style.display = "none";
            }
            if (selectionText) {
                selectionText.textContent = "";
            }
            if (selectionProgress && selectionProgressFill) {
                selectionProgress.style.display = "none";
                selectionProgressFill.style.width = "0%";
                if (selectionProgressTrack) {
                    selectionProgressTrack.setAttribute("aria-valuenow", "0");
                }
            }
        }

        const moveActiveFn = wasm_exports?.move_mode_active;
        const moveIsActive =
            typeof moveActiveFn === "function" && moveActiveFn() === 1;
        if (selectionMove) {
            selectionMove.classList.toggle("is-active", moveIsActive);
            selectionMove.setAttribute(
                "aria-pressed",
                moveIsActive ? "true" : "false"
            );
        }

        const useActiveFn = wasm_exports?.use_mode_active;
        const useIsActive =
            typeof useActiveFn === "function" && useActiveFn() === 1;
        if (selectionUse) {
            selectionUse.classList.toggle("is-active", useIsActive);
            selectionUse.setAttribute(
                "aria-pressed",
                useIsActive ? "true" : "false"
            );
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

