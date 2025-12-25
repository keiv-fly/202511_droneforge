function forwardZChange(callbackName) {
    const handler = wasm_exports?.[callbackName];
    if (typeof handler === "function") {
        handler();
    }
}

const selectionDecoder = new TextDecoder("utf-8");

const INVENTORY_SLOTS = 10;
const TILESET_SRC = "assets/tileset.png";
const tilesetImage = new Image();
tilesetImage.src = TILESET_SRC;
let tilesetReady = false;
tilesetImage.onload = () => {
    tilesetReady = true;
};

const BLOCK_NAME_BY_ID = {
    1: "dirt",
    2: "stone",
    3: "iron",
    4: "bedrock",
};

const describeInventorySlot = (blockId, count) => {
    if (!blockId || !count) {
        return "Empty slot";
    }
    const name = BLOCK_NAME_BY_ID[blockId] || `block ${blockId}`;
    return count > 1 ? `${name} x${count}` : name;
};

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
    const selectionTool = document.getElementById("selection-tool");
    const inventoryPanel = document.getElementById("inventory-panel");
    const inventoryGrid = document.getElementById("inventory-grid");
    const inventorySelection = document.getElementById("inventory-selection");
    const selectionProgress = document.getElementById("selection-progress");
    const selectionProgressTrack = document.getElementById(
        "selection-progress-track"
    );
    const selectionProgressFill = document.getElementById(
        "selection-progress-fill"
    );
    const inventoryCanvases = [];
    const inventoryCounts = [];
    const inventorySlots = [];
    const inventorySlotBlocks = Array(INVENTORY_SLOTS).fill(0);
    const inventorySlotCounts = Array(INVENTORY_SLOTS).fill(0);
    let lastSelectedSlot = null;
    let inventoryVisible = false;

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

    const clearInventorySelection = () => {
        lastSelectedSlot = null;
        if (inventorySelection) {
            inventorySelection.textContent = "";
        }
    };

    const updateInventorySelectionText = () => {
        if (!inventorySelection) {
            return;
        }
        if (lastSelectedSlot === null) {
            inventorySelection.textContent = "";
            return;
        }
        const block = inventorySlotBlocks[lastSelectedSlot] ?? 0;
        const count = inventorySlotCounts[lastSelectedSlot] ?? 0;
        inventorySelection.textContent = describeInventorySlot(block, count);
    };

    const handleInventorySlotClick = (slotIndex) => {
        lastSelectedSlot = slotIndex;
        updateInventorySelectionText();
    };

    if (inventoryGrid) {
        for (let i = 0; i < INVENTORY_SLOTS; i += 1) {
            const slot = document.createElement("div");
            slot.className = "inventory-slot";
            slot.tabIndex = 0;

            const canvas = document.createElement("canvas");
            canvas.className = "inventory-icon";
            canvas.width = 42;
            canvas.height = 42;

            const count = document.createElement("div");
            count.className = "inventory-count";

            slot.addEventListener("click", () => handleInventorySlotClick(i));
            slot.addEventListener("keydown", (event) => {
                if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    handleInventorySlotClick(i);
                }
            });

            slot.append(canvas, count);
            inventoryGrid.appendChild(slot);
            inventoryCanvases.push(canvas);
            inventoryCounts.push(count);
            inventorySlots.push(slot);
        }
    }

    const tileSizeFromWasm = () => {
        const fn = wasm_exports?.block_tile_size;
        return typeof fn === "function" ? fn() : 16;
    };

    const renderInventorySlots = () => {
        const blockFn = wasm_exports?.selected_drone_inventory_slot_block;
        const countFn = wasm_exports?.selected_drone_inventory_slot_count;
        const tileXFn = wasm_exports?.block_tile_pixel_x;
        const tileYFn = wasm_exports?.block_tile_pixel_y;
        const tileSize = tileSizeFromWasm();

        for (let i = 0; i < INVENTORY_SLOTS; i += 1) {
            const block =
                typeof blockFn === "function" ? blockFn(i) : 0;
            const count =
                typeof countFn === "function" ? countFn(i) : 0;
            const canvas = inventoryCanvases[i];
            const countEl = inventoryCounts[i];
            const slotEl = inventorySlots[i];

            inventorySlotBlocks[i] = block;
            inventorySlotCounts[i] = count;

            if (countEl) {
                countEl.textContent = count > 0 ? `${count}` : "";
            }

            if (slotEl) {
                slotEl.setAttribute("aria-label", describeInventorySlot(block, count));
            }

            if (!canvas) continue;
            const ctx = canvas.getContext("2d");
            if (!ctx) continue;

            ctx.clearRect(0, 0, canvas.width, canvas.height);
            if (!tilesetReady || block === 0 || count === 0) {
                continue;
            }

            const tileX =
                typeof tileXFn === "function" ? tileXFn(block) : -1;
            const tileY =
                typeof tileYFn === "function" ? tileYFn(block) : -1;

            if (tileX < 0 || tileY < 0) {
                continue;
            }

            ctx.imageSmoothingEnabled = false;
            ctx.drawImage(
                tilesetImage,
                tileX,
                tileY,
                tileSize,
                tileSize,
                0,
                0,
                canvas.width,
                canvas.height
            );
        }

        updateInventorySelectionText();
    };

    const updateInventoryVisibility = (isPresent) => {
        const shouldShow = inventoryVisible && isPresent;
        if (inventoryPanel) {
            inventoryPanel.classList.toggle("is-visible", shouldShow);
            inventoryPanel.setAttribute(
                "aria-hidden",
                shouldShow ? "false" : "true"
            );
        }
        if (selectionTool) {
            selectionTool.classList.toggle("is-active", shouldShow);
            selectionTool.setAttribute(
                "aria-pressed",
                shouldShow ? "true" : "false"
            );
        }
        if (!shouldShow) {
            clearInventorySelection();
        }
        if (shouldShow) {
            renderInventorySlots();
        }
    };

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

        updateInventoryVisibility(isPresent);

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

    if (selectionTool) {
        selectionTool.disabled = false;
        selectionTool.addEventListener("click", () => {
            inventoryVisible = !inventoryVisible;
            const presentFn = wasm_exports?.selected_drone_present;
            const hasSelection =
                typeof presentFn === "function" && presentFn() === 1;
            updateInventoryVisibility(hasSelection);
            if (inventoryVisible && hasSelection) {
                renderInventorySlots();
            }
        });
    }
});

