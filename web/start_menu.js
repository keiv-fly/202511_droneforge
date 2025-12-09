const WASM_WEIGHT = 0.4;
const CHUNK_WEIGHT = 0.6;
const WASM_PATH = "droneforge-web.wasm";

function isMobile() {
    const ua = navigator.userAgent || navigator.vendor || window.opera;
    if (/android/i.test(ua)) return true;
    if (/iPad|iPhone|iPod/.test(ua)) return true;
    if (window.matchMedia && matchMedia("(pointer: coarse)").matches) return true;
    return false;
}

async function requestFullscreenIfMobile() {
    if (!isMobile()) return;

    const element = document.documentElement;

    try {
        if (element.requestFullscreen) {
            await element.requestFullscreen();
        } else if (element.webkitRequestFullscreen) {
            element.webkitRequestFullscreen();
        } else if (element.msRequestFullscreen) {
            element.msRequestFullscreen();
        }
    } catch (error) {
        console.warn("Fullscreen request failed", error);
    }
}

function formatBytes(bytes) {
    if (!bytes) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
    const value = bytes / 1024 ** exponent;
    return `${value.toFixed(exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}

function combinedProgress(state) {
    return Math.min(
        1,
        state.wasmProgress * WASM_WEIGHT + state.chunkProgress * CHUNK_WEIGHT,
    );
}

function updateProgressUI(state, ui, stageText) {
    const overall = combinedProgress(state);
    const percent = Math.round(overall * 100);
    ui.progressFill.style.width = `${percent}%`;
    ui.progressTrack.setAttribute("aria-valuenow", String(percent));
    ui.loadingStatus.textContent = stageText;

    if (state.chunkProgress > 0) {
        if (state.chunkTotal > 0) {
            ui.progressDetail.textContent = `Caching world ${state.chunkLoaded.toLocaleString()} / ${state.chunkTotal.toLocaleString()}`;
        } else {
            ui.progressDetail.textContent = `Caching world ${(state.chunkProgress * 100).toFixed(1)}%`;
        }
    } else if (state.wasmProgress < 1) {
        if (state.wasmTotalBytes > 0) {
            ui.progressDetail.textContent = `Downloading ${formatBytes(state.wasmLoadedBytes)} / ${formatBytes(state.wasmTotalBytes)}`;
        } else {
            ui.progressDetail.textContent = "Downloading game files…";
        }
    } else {
        ui.progressDetail.textContent = "Starting simulation…";
    }
}

function markReady(state, ui) {
    if (state.ready) return;
    state.ready = true;
    state.wasmProgress = 1;
    state.chunkProgress = 1;
    updateProgressUI(state, ui, "Ready to start");
    ui.progressDetail.textContent = "World cached. Press Start to play.";
    ui.startButton.disabled = false;
    ui.startButton.textContent = "Start";

    if (state.startClicked) {
        ui.startScreen.style.display = "none";
    }
}

async function fetchWasmWithProgress(url, onProgress) {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`Download failed: ${response.status} ${response.statusText}`);
    }

    const total = Number(response.headers.get("content-length")) || 0;
    if (!response.body || total === 0) {
        const buffer = await response.arrayBuffer();
        onProgress(buffer.byteLength, buffer.byteLength);
        return buffer;
    }

    const reader = response.body.getReader();
    let received = 0;
    const chunks = [];

    while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        chunks.push(value);
        received += value.length;
        onProgress(received, total);
    }

    const full = new Uint8Array(received);
    let offset = 0;
    for (const chunk of chunks) {
        full.set(chunk, offset);
        offset += chunk.length;
    }

    return full.buffer;
}

async function instantiateWasm(bytes) {
    register_plugins(plugins);
    const module = await WebAssembly.compile(bytes);
    add_missing_functions_stabs(module);
    const instance = await WebAssembly.instantiate(module, importObject);

    wasm_memory = instance.exports.memory;
    wasm_exports = instance.exports;

    if (typeof wasm_exports.crate_version === "function") {
        const crateVersion = wasm_exports.crate_version();
        if (version !== crateVersion) {
            console.error(`Version mismatch: gl.js=${version}, crate=${crateVersion}`);
        }
    }

    init_plugins(plugins);
    instance.exports.main();
}

function startChunkPolling(state, ui) {
    const poll = () => {
        if (!wasm_exports) {
            requestAnimationFrame(poll);
            return;
        }

        const totalFn = wasm_exports.chunk_cache_total_chunks;
        const loadedFn = wasm_exports.chunk_cache_loaded_chunks;
        const fractionFn = wasm_exports.chunk_cache_progress_fraction;

        const total = typeof totalFn === "function" ? totalFn() : 0;
        const loaded = typeof loadedFn === "function" ? loadedFn() : 0;
        const fraction = typeof fractionFn === "function" ? fractionFn() : 0;

        if (total > 0) {
            state.chunkTotal = total;
            state.chunkLoaded = Math.min(loaded, total);
            state.chunkProgress = state.chunkLoaded / total;
        } else {
            state.chunkProgress = fraction;
        }

        updateProgressUI(state, ui, "Caching world data");

        if (state.chunkProgress >= 0.999 && (total === 0 || state.chunkLoaded >= total)) {
            markReady(state, ui);
        } else {
            requestAnimationFrame(poll);
        }
    };

    poll();
}

async function startLoading(state, ui) {
    try {
        const wasmBytes = await fetchWasmWithProgress(WASM_PATH, (loaded, total) => {
            state.wasmLoadedBytes = loaded;
            state.wasmTotalBytes = total;
            state.wasmProgress = total > 0 ? loaded / total : 0.2;
            updateProgressUI(state, ui, "Downloading game");
        });

        state.wasmProgress = 1;
        updateProgressUI(state, ui, "Initializing game");

        await instantiateWasm(wasmBytes);
        startChunkPolling(state, ui);
    } catch (error) {
        console.error("Failed to load game", error);
        ui.loadingStatus.textContent = "Failed to load game";
        ui.progressDetail.textContent =
            error?.message || "Check your connection and try again.";
        ui.progressDetail.classList.add("loading-error");
        ui.startButton.disabled = true;
        ui.startButton.textContent = "Reload to retry";
    }
}

window.addEventListener("load", () => {
    const ui = {
        startScreen: document.getElementById("start-screen"),
        startButton: document.getElementById("start-button"),
        loadingStatus: document.getElementById("loading-status"),
        progressTrack: document.querySelector(".progress-track"),
        progressFill: document.getElementById("progress-fill"),
        progressDetail: document.getElementById("progress-detail"),
    };

    const state = {
        wasmProgress: 0,
        wasmLoadedBytes: 0,
        wasmTotalBytes: 0,
        chunkProgress: 0,
        chunkLoaded: 0,
        chunkTotal: 0,
        ready: false,
        startClicked: false,
    };

    updateProgressUI(state, ui, "Preparing download...");
    ui.startButton.disabled = true;
    ui.startButton.textContent = "Loading…";

    ui.startButton.addEventListener("click", async () => {
        state.startClicked = true;
        ui.startButton.disabled = true;
        ui.startButton.textContent = "Loading…";
        await requestFullscreenIfMobile();

        if (state.ready) {
            ui.startScreen.style.display = "none";
        }
    });

    startLoading(state, ui);
});
