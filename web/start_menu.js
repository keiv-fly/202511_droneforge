const WASM_PATH = "droneforge-web.wasm";

function createBenchMetrics() {
    const htmlStart = window.gameHtmlStart || performance.now();
    window.gameHtmlStart = htmlStart;
    return {
        gameHtmlStart: htmlStart,
        gameLoadStart: null,
        gameReadyAt: null,
        firstFrameAt: null,
        firstFrameDelta: null,
        firstFps: null,
        chunkLoadingTime: null,
        renderCaching5: null,
        avgChunkLoad: null,
    };
}

function publishBenchMetrics(metrics) {
    window.droneforgeMetrics = { ...metrics };
}

function readWasmMetric(fnName) {
    const fn = wasm_exports?.[fnName];
    if (typeof fn !== "function") {
        return null;
    }
    const value = fn();
    return Number.isFinite(value) ? value : null;
}

function captureReadyMetrics(state) {
    state.metrics.chunkLoadingTime = readWasmMetric("bench_initial_chunk_cache_ms");
    state.metrics.renderCaching5 = readWasmMetric("bench_initial_render_cache_ms");
    state.metrics.avgChunkLoad = readWasmMetric("bench_avg_chunk_load_ms");
    publishBenchMetrics(state.metrics);
}

function beginFirstFrameProbe(state) {
    if (state.firstFrameProbeStarted) return;
    state.firstFrameProbeStarted = true;

    requestAnimationFrame((firstTimestamp) => {
        const firstFrameAt = performance.now();
        if (!state.metrics.firstFrameAt) {
            state.metrics.firstFrameAt = firstFrameAt;
            window.firstFrameAt = firstFrameAt;
        }

        requestAnimationFrame((secondTimestamp) => {
            const delta = secondTimestamp - firstTimestamp;
            if (!state.metrics.firstFrameDelta) {
                state.metrics.firstFrameDelta = delta;
                window.firstFrameDelta = delta;
            }

            if (!state.metrics.firstFps && delta > 0) {
                state.metrics.firstFps = 1000 / delta;
                window.firstFps = state.metrics.firstFps;
            }

            publishBenchMetrics(state.metrics);
        });
    });
}

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
    return state.ready ? 1 : 0;
}

function updateProgressUI(state, ui, stageText) {
    const overall = combinedProgress(state);
    const percent = Math.round(overall * 100);
    ui.progressFill.style.width = `${percent}%`;
    ui.progressTrack.setAttribute("aria-valuenow", String(percent));
    ui.loadingStatus.textContent = stageText;

    if (state.ready) {
        ui.progressDetail.textContent = "Ready. Press Start to play.";
    } else if (state.wasmProgress < 1) {
        if (state.wasmTotalBytes > 0) {
            ui.progressDetail.textContent = `Downloading ${formatBytes(state.wasmLoadedBytes)} / ${formatBytes(state.wasmTotalBytes)}`;
        } else {
            ui.progressDetail.textContent = "Downloading game files…";
        }
    } else {
        ui.progressDetail.textContent = "Initializing game…";
    }
}

function markReady(state, ui) {
    if (state.ready) return;
    state.ready = true;
    state.wasmProgress = 1;
    updateProgressUI(state, ui, "Ready to start");
    ui.progressDetail.textContent = "Ready. Press Start to play.";
    ui.startButton.disabled = false;
    ui.startButton.textContent = "Start";

    if (!state.metrics.gameReadyAt) {
        const readyAt = performance.now();
        state.metrics.gameReadyAt = readyAt;
        window.gameReadyAt = readyAt;
    }

    window.gameReady = true;

    if (!state.metricsCaptured) {
        state.metricsCaptured = true;
        captureReadyMetrics(state);
    }
    publishBenchMetrics(state.metrics);

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
        markReady(state, ui);
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
    window.gameReady = false;

    const ui = {
        startScreen: document.getElementById("start-screen"),
        startButton: document.getElementById("start-button"),
        loadingStatus: document.getElementById("loading-status"),
        progressTrack: document.querySelector(".progress-track"),
        progressFill: document.getElementById("progress-fill"),
        progressDetail: document.getElementById("progress-detail"),
    };

    const state = {
        metrics: createBenchMetrics(),
        metricsCaptured: false,
        wasmProgress: 0,
        wasmLoadedBytes: 0,
        wasmTotalBytes: 0,
        ready: false,
        startClicked: false,
        firstFrameProbeStarted: false,
    };

    state.metrics.gameLoadStart = performance.now();
    window.gameLoadStart = state.metrics.gameLoadStart;
    publishBenchMetrics(state.metrics);

    updateProgressUI(state, ui, "Preparing download...");
    ui.startButton.disabled = true;
    ui.startButton.textContent = "Loading…";

    ui.startButton.addEventListener("click", async () => {
        state.startClicked = true;
        ui.startButton.disabled = true;
        ui.startButton.textContent = "Loading…";
        await requestFullscreenIfMobile();
        beginFirstFrameProbe(state);

        if (state.ready) {
            ui.startScreen.style.display = "none";
        }
    });

    startLoading(state, ui);
});
