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

const STONE_BLOCK_ID = 2;
let cachedCoreBlockId = null;

const resolveCoreBlockId = () => {
    const fn = wasm_exports?.core_block_id;
    if (typeof fn === "function") {
        cachedCoreBlockId = fn();
    } else if (cachedCoreBlockId === null) {
        cachedCoreBlockId = 5;
    }
    return cachedCoreBlockId;
};

const ensureCoreNameRegistered = () => {
    const coreId = resolveCoreBlockId();
    if (coreId && !BLOCK_NAME_BY_ID[coreId]) {
        BLOCK_NAME_BY_ID[coreId] = "core";
    }
};

const describeInventorySlot = (blockId, count) => {
    ensureCoreNameRegistered();
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
    let toolPreviewCanvas = null;
    let toolPreviewCtx = null;
    let lastToolBlock = 0;
    let lastToolCount = 0;
    let inventoryActionMenu = null;
    let inventoryActionSlot = null;
    let longPressTimer = null;
    let suppressNextSlotClick = false;

    if (canvas) {
        canvas.addEventListener("contextmenu", (event) => {
            event.preventDefault();
        });
    }

    document.addEventListener("pointerdown", (event) => {
        if (!inventoryActionMenu || !inventoryActionMenu.classList.contains("is-visible")) {
            return;
        }
        if (inventoryActionMenu.contains(event.target)) {
            return;
        }
        closeInventoryActionMenu();
    });

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

    const selectToolSlot = (slotIndex) => {
        const block = inventorySlotBlocks[slotIndex] ?? 0;
        const count = inventorySlotCounts[slotIndex] ?? 0;
        if (count > 0) {
            const fn = wasm_exports?.tool_select_slot;
            if (typeof fn === "function") {
                fn(slotIndex);
            }
        } else {
            const clearFn = wasm_exports?.tool_clear_selection;
            if (typeof clearFn === "function") {
                clearFn();
            }
        }
    };

    const handleInventorySlotClick = (slotIndex) => {
        if (suppressNextSlotClick) {
            suppressNextSlotClick = false;
            return;
        }
        closeInventoryActionMenu();
        lastSelectedSlot = slotIndex;
        selectToolSlot(slotIndex);
        updateInventorySelectionText();
        renderToolPreview();
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
            slot.addEventListener("contextmenu", (event) => {
                event.preventDefault();
                openInventoryActionMenu(i, slot);
            });
            slot.addEventListener("pointerdown", (event) => {
                if (event.pointerType === "touch") {
                    clearLongPressTimer();
                    longPressTimer = window.setTimeout(() => {
                        longPressTimer = null;
                        openInventoryActionMenu(i, slot);
                    }, 550);
                }
            });
            const cancelLongPress = () => clearLongPressTimer();
            slot.addEventListener("pointerup", cancelLongPress);
            slot.addEventListener("pointerleave", cancelLongPress);
            slot.addEventListener("pointercancel", cancelLongPress);

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

    const drawCoreIcon = (ctx, size) => {
        if (!ctx) return;
        const radius = Math.max(1, Math.floor(size * 0.5 - size * 0.2));
        const cx = size / 2;
        const cy = size / 2;
        ctx.fillStyle = "#003592";
        ctx.beginPath();
        ctx.moveTo(cx, cy - radius);
        ctx.lineTo(cx + radius, cy);
        ctx.lineTo(cx, cy + radius);
        ctx.lineTo(cx - radius, cy);
        ctx.closePath();
        ctx.fill();
    };

    const clearLongPressTimer = () => {
        if (longPressTimer) {
            clearTimeout(longPressTimer);
            longPressTimer = null;
        }
    };

    const createInventoryActionMenu = () => {
        if (inventoryActionMenu) return;
        const menu = document.createElement("div");
        menu.id = "inventory-action-menu";
        menu.className = "inventory-action-menu";
        const actionBtn = document.createElement("button");
        actionBtn.type = "button";
        actionBtn.className = "inventory-action-btn";
        actionBtn.textContent = "create core";
        actionBtn.addEventListener("click", () => {
            if (inventoryActionSlot !== null) {
                const fn = wasm_exports?.inventory_create_core;
                if (typeof fn === "function") {
                    fn(inventoryActionSlot);
                }
                closeInventoryActionMenu();
                renderInventorySlots();
            }
        });
        menu.appendChild(actionBtn);
        document.body.appendChild(menu);
        inventoryActionMenu = menu;
    };

    const closeInventoryActionMenu = () => {
        if (!inventoryActionMenu) return;
        inventoryActionMenu.classList.remove("is-visible");
        inventoryActionMenu.style.display = "none";
        inventoryActionSlot = null;
        suppressNextSlotClick = false;
    };

    const positionInventoryActionMenu = (anchor) => {
        if (!inventoryActionMenu || !anchor) return;
        const rect = anchor.getBoundingClientRect();
        inventoryActionMenu.style.display = "flex";
        inventoryActionMenu.style.visibility = "hidden";
        const { offsetWidth, offsetHeight } = inventoryActionMenu;
        const margin = 8;
        const width = offsetWidth || 160;
        const height = offsetHeight || 60;
        const left = Math.min(
            window.innerWidth - width - margin,
            rect.right + margin
        );
        const top = Math.min(
            window.innerHeight - height - margin,
            rect.top
        );
        inventoryActionMenu.style.left = `${Math.max(margin, left)}px`;
        inventoryActionMenu.style.top = `${Math.max(margin, top)}px`;
        inventoryActionMenu.style.visibility = "visible";
    };

    const openInventoryActionMenu = (slotIndex, anchor) => {
        createInventoryActionMenu();
        const block = inventorySlotBlocks[slotIndex] ?? 0;
        const count = inventorySlotCounts[slotIndex] ?? 0;
        if (block !== STONE_BLOCK_ID || count <= 0) {
            closeInventoryActionMenu();
            return;
        }
        inventoryActionSlot = slotIndex;
        suppressNextSlotClick = true;
        positionInventoryActionMenu(anchor);
        inventoryActionMenu.classList.add("is-visible");
    };

    const ensureToolPreview = () => {
        if (toolPreviewCanvas || !selectionTool) {
            return;
        }
        toolPreviewCanvas = document.createElement("canvas");
        toolPreviewCanvas.className = "tool-preview";
        toolPreviewCanvas.width = 28;
        toolPreviewCanvas.height = 28;
        toolPreviewCtx = toolPreviewCanvas.getContext("2d");
        selectionTool.appendChild(toolPreviewCanvas);
    };

    const renderToolPreview = () => {
        ensureToolPreview();
        if (!toolPreviewCanvas || !toolPreviewCtx) {
            return;
        }

        const blockFn = wasm_exports?.selected_tool_block;
        const countFn = wasm_exports?.selected_tool_count;
        const tileXFn = wasm_exports?.block_tile_pixel_x;
        const tileYFn = wasm_exports?.block_tile_pixel_y;
        const tileSize = tileSizeFromWasm();

        const block = typeof blockFn === "function" ? blockFn() : 0;
        const count = typeof countFn === "function" ? countFn() : 0;
        const isCoreBlock = block === resolveCoreBlockId();

        if (block === lastToolBlock && count === lastToolCount) {
            return;
        }
        lastToolBlock = block;
        lastToolCount = count;

        toolPreviewCtx.clearRect(
            0,
            0,
            toolPreviewCanvas.width,
            toolPreviewCanvas.height
        );
        selectionTool?.classList.toggle("has-tool", block !== 0 && count > 0);

        if (block === 0 || count === 0) {
            return;
        }

        if (isCoreBlock) {
            drawCoreIcon(
                toolPreviewCtx,
                Math.min(toolPreviewCanvas.width, toolPreviewCanvas.height)
            );
            return;
        }

        if (!tilesetReady) {
            return;
        }

        const tileX =
            typeof tileXFn === "function" ? tileXFn(block) : -1;
        const tileY =
            typeof tileYFn === "function" ? tileYFn(block) : -1;

        if (tileX < 0 || tileY < 0) {
            return;
        }

        toolPreviewCtx.imageSmoothingEnabled = false;
        toolPreviewCtx.drawImage(
            tilesetImage,
            tileX,
            tileY,
            tileSize,
            tileSize,
            0,
            0,
            toolPreviewCanvas.width,
            toolPreviewCanvas.height
        );
    };

    const renderInventorySlots = () => {
        ensureCoreNameRegistered();
        const blockFn = wasm_exports?.selected_drone_inventory_slot_block;
        const countFn = wasm_exports?.selected_drone_inventory_slot_count;
        const tileXFn = wasm_exports?.block_tile_pixel_x;
        const tileYFn = wasm_exports?.block_tile_pixel_y;
        const tileSize = tileSizeFromWasm();
        const coreBlockId = resolveCoreBlockId();

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
            if (block === 0 || count === 0) {
                continue;
            }

            if (block === coreBlockId) {
                drawCoreIcon(ctx, Math.min(canvas.width, canvas.height));
                continue;
            }

            if (!tilesetReady) {
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
        renderToolPreview();

        if (inventoryActionSlot !== null) {
            const activeBlock = inventorySlotBlocks[inventoryActionSlot] ?? 0;
            const activeCount = inventorySlotCounts[inventoryActionSlot] ?? 0;
            if (activeBlock !== STONE_BLOCK_ID || activeCount === 0) {
                closeInventoryActionMenu();
            }
        }
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
            closeInventoryActionMenu();
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
        renderToolPreview();
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

