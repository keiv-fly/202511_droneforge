#!/usr/bin/env node
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawn } = require("child_process");

const METRIC_DEFS = {
    loadScreenBoot: { label: "Load screen boot", unit: "ms" },
    loadingScreen: { label: "Loading screen", unit: "ms" },
    loadingFirstFrame: { label: "Loading to first frame", unit: "ms" },
    chunkLoadingTime: { label: "Chunk cache build", unit: "ms" },
    renderCaching5: { label: "Render cache ±5", unit: "ms" },
    avgChunkLoad: { label: "Average chunk load", unit: "ms" },
    firstFrameDelta: { label: "First frame delta", unit: "ms" },
    firstFps: { label: "First frame FPS", unit: "fps" },
};

function parseArgs(argv) {
    const acc = {};
    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === "--") continue;
        if (!arg.startsWith("--")) continue;

        const withoutPrefix = arg.replace(/^--/, "");
        if (withoutPrefix.length === 0) continue;
        if (withoutPrefix.includes("=")) {
            const [key, rawValue] = withoutPrefix.split("=", 2);
            acc[key] = rawValue === undefined ? true : rawValue;
            continue;
        }

        const key = withoutPrefix;
        const next = argv[i + 1];
        if (next && !next.startsWith("--")) {
            acc[key] = next;
            i += 1;
        } else {
            acc[key] = true;
        }
    }
    return acc;
}

function spawnPromise(cmd, args, options = {}) {
    return new Promise((resolve, reject) => {
        const child = spawn(cmd, args, { stdio: "inherit", ...options });
        child.on("error", reject);
        child.on("exit", (code) => {
            if (code === 0) {
                resolve();
            } else {
                reject(new Error(`${cmd} exited with code ${code}`));
            }
        });
    });
}

async function killPortIfListening(port) {
    const isWin = process.platform === "win32";
    if (isWin) {
        const script = `Get-NetTCPConnection -State Listen -LocalPort ${port} | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force }`;
        try {
            await spawnPromise("powershell", ["-NoProfile", "-Command", script]);
        } catch (_error) {
            // Ignore inability to kill; server start may still succeed.
        }
    } else {
        const cmd = `lsof -ti tcp:${port} | xargs -r kill -9`;
        try {
            await spawnPromise("sh", ["-c", cmd]);
        } catch (_error) {
            // Ignore inability to kill; server start may still succeed.
        }
    }
}

function ensureDir(dir) {
    fs.mkdirSync(dir, { recursive: true });
}

function isNumber(value) {
    return Number.isFinite(value);
}

function toNumber(value) {
    return isNumber(value) ? value : null;
}

function diff(a, b) {
    return isNumber(a) && isNumber(b) ? a - b : null;
}

function percentile(sortedValues, percentileRank) {
    if (!sortedValues.length) return null;
    const index = (sortedValues.length - 1) * percentileRank;
    const lower = Math.floor(index);
    const upper = Math.ceil(index);
    if (lower === upper) return sortedValues[lower];
    const weight = index - lower;
    return sortedValues[lower] * (1 - weight) + sortedValues[upper] * weight;
}

function summarize(values, unit) {
    if (!values.length) {
        return {
            unit,
            count: 0,
            mean: null,
            median: null,
            p95: null,
            min: null,
            max: null,
            stddev: null,
        };
    }

    const sorted = [...values].sort((a, b) => a - b);
    const mean = sorted.reduce((acc, value) => acc + value, 0) / sorted.length;
    const mid = Math.floor(sorted.length / 2);
    const median =
        sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];
    const variance =
        sorted.reduce((acc, value) => acc + Math.pow(value - mean, 2), 0) /
        sorted.length;
    return {
        unit,
        count: sorted.length,
        mean,
        median,
        p95: percentile(sorted, 0.95),
        min: sorted[0],
        max: sorted[sorted.length - 1],
        stddev: Math.sqrt(variance),
    };
}

function buildAggregates(runs) {
    const aggregates = {};
    for (const [key, meta] of Object.entries(METRIC_DEFS)) {
        const values = runs
            .map((run) => run?.values?.[key])
            .filter((value) => isNumber(value));
        aggregates[key] = summarize(values, meta.unit);
    }
    return aggregates;
}

function computeRun(runIndex, metrics) {
    const points = {
        gameHtmlStart: toNumber(metrics.gameHtmlStart),
        gameLoadStart: toNumber(metrics.gameLoadStart),
        gameReadyAt: toNumber(metrics.gameReadyAt),
        firstFrameAt: toNumber(metrics.firstFrameAt),
    };

    const values = {
        loadScreenBoot: diff(points.gameLoadStart, points.gameHtmlStart),
        loadingScreen: diff(points.gameReadyAt, points.gameLoadStart),
        loadingFirstFrame: diff(points.firstFrameAt, points.gameReadyAt),
        firstFrameDelta: toNumber(metrics.firstFrameDelta),
        firstFps:
            toNumber(metrics.firstFps) ||
            (isNumber(metrics.firstFrameDelta) && metrics.firstFrameDelta > 0
                ? 1000 / metrics.firstFrameDelta
                : null),
        chunkLoadingTime: toNumber(metrics.chunkLoadingTime),
        renderCaching5: toNumber(metrics.renderCaching5),
        avgChunkLoad: toNumber(metrics.avgChunkLoad),
    };

    return { run: runIndex, points, values };
}

function mulberry32(seed) {
    let value = seed >>> 0;
    return function next() {
        value |= 0;
        value = (value + 0x6d2b79f5) | 0;
        let t = Math.imul(value ^ (value >>> 15), 1 | value);
        t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
        return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
}

function generateMockRun(runIndex) {
    const rng = mulberry32(42 + runIndex);
    const jitter = (base, spread) => base + rng() * spread;

    const loadScreenBoot = jitter(40, 25);
    const loadingScreen = jitter(800, 200);
    const loadingFirstFrame = jitter(40, 25);
    const chunkLoadingTime = jitter(450, 80);
    const renderCaching5 = jitter(380, 90);
    const avgChunkLoad = jitter(12, 6);
    const firstFrameDelta = jitter(16, 4);
    const firstFps = firstFrameDelta > 0 ? 1000 / firstFrameDelta : null;

    const gameHtmlStart = 0;
    const gameLoadStart = gameHtmlStart + loadScreenBoot;
    const gameReadyAt = gameLoadStart + loadingScreen;
    const firstFrameAt = gameReadyAt + loadingFirstFrame;

    return computeRun(runIndex, {
        gameHtmlStart,
        gameLoadStart,
        gameReadyAt,
        firstFrameAt,
        firstFrameDelta,
        firstFps,
        chunkLoadingTime,
        renderCaching5,
        avgChunkLoad,
    });
}

async function collectRealRun({ context, url, navTimeout, runIndex, setUserAgent }) {
    const page = await context.newPage();
    try {
        await page.goto(url, { waitUntil: "networkidle", timeout: navTimeout });

        if (setUserAgent) {
            const ua = await page.evaluate(() => navigator.userAgent);
            setUserAgent(ua);
        }

        await page.waitForFunction(() => typeof window.gameLoadStart === "number", {
            timeout: navTimeout,
        });
        await page.waitForFunction(() => window.gameReady === true, { timeout: navTimeout });
        await page.waitForSelector("#start-button:not([disabled])", {
            timeout: navTimeout,
        });
        await page.click("#start-button");
        await page.waitForFunction(
            () => {
                const metrics = window.droneforgeMetrics;
                if (!metrics) return false;
                return (
                    typeof metrics.firstFrameAt === "number" &&
                    typeof metrics.firstFrameDelta === "number"
                );
            },
            { timeout: navTimeout }
        );

        const rawMetrics = await page.evaluate(() => {
            const metrics = window.droneforgeMetrics || {};
            return {
                gameHtmlStart: metrics.gameHtmlStart ?? null,
                gameLoadStart: metrics.gameLoadStart ?? null,
                gameReadyAt: metrics.gameReadyAt ?? null,
                firstFrameAt: metrics.firstFrameAt ?? null,
                firstFrameDelta: metrics.firstFrameDelta ?? null,
                firstFps: metrics.firstFps ?? null,
                chunkLoadingTime: metrics.chunkLoadingTime ?? null,
                renderCaching5: metrics.renderCaching5 ?? null,
                avgChunkLoad: metrics.avgChunkLoad ?? null,
            };
        });

        return computeRun(runIndex, rawMetrics);
    } catch (error) {
        return { run: runIndex, error: error?.message || String(error), points: {}, values: {} };
    } finally {
        await page.close();
    }
}

function formatNumber(value) {
    if (!isNumber(value)) return "-";
    return value.toFixed(2);
}

function renderMarkdown(payload) {
    const lines = [];
    const env = payload.environment;
    lines.push("# Droneforge load benchmark");
    lines.push("");
    lines.push(`- Mode: ${payload.mode}`);
    lines.push(`- Runs: ${payload.runs}`);
    lines.push(`- URL: ${payload.url}`);
    lines.push(`- Generated: ${payload.generatedAt}`);
    lines.push(
        `- OS: ${env.os.platform} ${env.os.release}${env.os.version ? ` (${env.os.version})` : ""} ${env.os.arch}`
    );
    lines.push(
        `- Browser: ${env.browser.name}${env.browser.version ? ` ${env.browser.version}` : ""}${
            env.browser.userAgent ? ` | ${env.browser.userAgent}` : ""
        }`
    );
    lines.push(
        `- Processor: ${env.cpu.model || "unknown"}${env.cpu.cores ? ` (${env.cpu.cores} cores)` : ""}`
    );
    lines.push("");
    lines.push("## Aggregated metrics (ms unless noted)");
    lines.push("| metric | mean | median | p95 | min | max | unit |");
    lines.push("| --- | --- | --- | --- | --- | --- | --- |");
    for (const [key, meta] of Object.entries(METRIC_DEFS)) {
        const agg = payload.aggregates[key];
        lines.push(
            `| ${meta.label} | ${formatNumber(agg.mean)} | ${formatNumber(agg.median)} | ${formatNumber(
                agg.p95
            )} | ${formatNumber(agg.min)} | ${formatNumber(agg.max)} | ${meta.unit} |`
        );
    }

    lines.push("");
    lines.push("## Per-run metrics");
    lines.push(
        "| run | loadScreenBoot | loadingScreen | loadingFirstFrame | chunkLoadingTime | renderCaching5 | avgChunkLoad | firstFps |"
    );
    lines.push("| --- | --- | --- | --- | --- | --- | --- | --- |");
    payload.metrics.perRun.forEach((run) => {
        if (run.error) {
            lines.push(`| ${run.run} | error: ${run.error} | - | - | - | - | - | - |`);
            return;
        }
        lines.push(
            `| ${run.run} | ${formatNumber(run.values.loadScreenBoot)} | ${formatNumber(
                run.values.loadingScreen
            )} | ${formatNumber(run.values.loadingFirstFrame)} | ${formatNumber(
                run.values.chunkLoadingTime
            )} | ${formatNumber(run.values.renderCaching5)} | ${formatNumber(
                run.values.avgChunkLoad
            )} | ${formatNumber(run.values.firstFps)} |`
        );
    });

    lines.push("");
    lines.push(
        "Values are in milliseconds except `firstFps`, which is frames per second. Results are generated by `benchmark/run.js`."
    );
    return lines.join("\n");
}

function collectEnvironment(url) {
    const cpuInfo = os.cpus()?.[0];
    return {
        url,
        os: {
            platform: os.platform(),
            release: os.release(),
            arch: os.arch(),
            version: typeof os.version === "function" ? os.version() : undefined,
        },
        browser: {
            name: "chromium",
            version: null,
            userAgent: null,
        },
        cpu: cpuInfo
            ? { model: cpuInfo.model, speedMHz: cpuInfo.speed, cores: os.cpus().length }
            : { model: null, speedMHz: null, cores: null },
    };
}

function renderProgress(current, total) {
    const width = 24;
    const clamped = Math.max(0, Math.min(current, total));
    const filled = Math.round((clamped / total) * width);
    const empty = width - filled;
    const bar = `[${"#".repeat(filled)}${"-".repeat(empty)}]`;
    process.stdout.write(`\rRuns ${bar} ${clamped}/${total}`);
    if (clamped === total) {
        process.stdout.write("\n");
    }
}

function normalizeTargetUrl(rawUrl) {
    if (!rawUrl) return rawUrl;
    try {
        const u = new URL(rawUrl);
        const segments = u.pathname.split("/").filter(Boolean);
        const last = segments[segments.length - 1] || "";
        const hasFile = last.includes(".");

        if (u.pathname.endsWith("/")) {
            u.pathname = `${u.pathname}index.html`;
        } else if (!hasFile) {
            u.pathname = `${u.pathname}/index.html`;
        }
        return u.toString();
    } catch (_error) {
        return rawUrl;
    }
}

async function prepareWasmAndServer({ port, rootDir }) {
    const targetTriple = "wasm32-unknown-unknown";
    const wasmSource = path.join(rootDir, "target", targetTriple, "release", "droneforge-web.wasm");
    const webDir = path.join(rootDir, "web");
    const wasmDest = path.join(webDir, "droneforge-web.wasm");

    console.log("Building WASM (cargo build -p droneforge-web --release --target wasm32-unknown-unknown)...");
    await spawnPromise("cargo", ["build", "-p", "droneforge-web", "--release", "--target", targetTriple], {
        cwd: rootDir,
    });

    if (!fs.existsSync(wasmSource)) {
        throw new Error(`WASM build output not found at ${wasmSource}`);
    }

    ensureDir(webDir);
    fs.copyFileSync(wasmSource, wasmDest);
    console.log(`Copied wasm to ${wasmDest}`);

    console.log(`Ensuring port ${port} is free...`);
    await killPortIfListening(port);

    console.log(`Starting simple-http-server on port ${port}...`);
    const server = spawn("simple-http-server", [".", "-p", String(port)], {
        cwd: webDir,
        stdio: "inherit",
    });

    await new Promise((resolve) => setTimeout(resolve, 1000));
    return { server, webDir };
}

function collectAllCliArgs() {
    const direct = process.argv.slice(2);
    let fromNpm = [];
    try {
        const parsed = JSON.parse(process.env.npm_config_argv || "");
        if (parsed) {
            const cooked = Array.isArray(parsed.cooked) ? parsed.cooked : [];
            const dashDash = cooked.indexOf("--");
            if (dashDash >= 0) {
                fromNpm = cooked.slice(dashDash + 1);
            } else {
                // drop the npm command and script name if present
                fromNpm = cooked.slice(2);
            }
        }
    } catch (_error) {
        // ignore
    }
    return [...direct, ...fromNpm];
}

function debugLogArgs() {
    if (process.env.DF_DEBUG_ARGS !== "1") {
        return;
    }
    console.log("process.argv:", process.argv);
    console.log("npm_config_argv:", process.env.npm_config_argv);
}

function firstPositional(argv) {
    for (const arg of argv) {
        if (!arg.startsWith("--")) {
            return arg;
        }
    }
    return null;
}

async function main() {
    const rawArgs = collectAllCliArgs();
    const args = parseArgs(rawArgs);
    debugLogArgs();
    const useMock = Boolean(args.mock || process.env.DF_BENCH_MOCK);
    const runs =
        Number.parseInt(args.runs || process.env.DF_BENCH_RUNS || "10", 10) ||
        10;
    const defaultPort = Number.parseInt(process.env.DF_BENCH_PORT || "8005", 10) || 8005;
    const shouldPrepare = !(args["skip-prepare"] || process.env.DF_BENCH_SKIP_PREPARE);
    const rootDir = path.resolve(__dirname, "..");
    let serverProcess = null;
    let targetUrl = normalizeTargetUrl(
        args.url ||
            firstPositional(rawArgs) ||
            process.env.DF_BENCH_URL ||
            `http://127.0.0.1:${defaultPort}/index.html`
    );
    const navTimeout =
        Number.parseInt(args.timeout || process.env.DF_BENCH_TIMEOUT || "120000", 10) ||
        120000;

    const resultsDir = path.join(__dirname, "results");
    ensureDir(resultsDir);
    const jsonPath = path.join(resultsDir, "load-benchmark.json");
    const mdPath = path.join(resultsDir, "load-benchmark.md");

    const environment = collectEnvironment(targetUrl);
    const perRun = [];
    let chromium;
    let browserVersion = null;
    let browserName = "chromium";
    let userAgent = null;

    try {
        if (shouldPrepare && !useMock) {
            const { server } = await prepareWasmAndServer({ port: defaultPort, rootDir });
            serverProcess = server;
            targetUrl = normalizeTargetUrl(args.url || `http://127.0.0.1:${defaultPort}/index.html`);
        }

        if (useMock) {
            renderProgress(0, runs);
            for (let runIndex = 1; runIndex <= runs; runIndex += 1) {
                perRun.push(generateMockRun(runIndex));
                renderProgress(runIndex, runs);
            }
            browserName = "mock";
            browserVersion = "mock";
        } else {
            try {
                ({ chromium } = require("playwright"));
            } catch (error) {
                console.error(
                    "Playwright is not installed. Run `npm install` inside the benchmark folder."
                );
                process.exit(1);
            }

            const browser = await chromium.launch({
                headless: true,
                args: ["--disable-gpu", "--disable-dev-shm-usage"],
            });
            browserVersion = browser.version();
            browserName = typeof chromium.name === "function" ? chromium.name() : "chromium";

            const context = await browser.newContext({
                viewport: { width: 1280, height: 720 },
                bypassCSP: true,
            });

            renderProgress(0, runs);
            for (let runIndex = 1; runIndex <= runs; runIndex += 1) {
                const result = await collectRealRun({
                    context,
                    url: targetUrl,
                    navTimeout,
                    runIndex,
                    setUserAgent: (ua) => {
                        if (!userAgent) {
                            userAgent = ua;
                        }
                    },
                });
                perRun.push(result);
                renderProgress(runIndex, runs);
            }

            await browser.close();
        }

        environment.browser.name = browserName;
        environment.browser.version = browserVersion;
        environment.browser.userAgent = userAgent;

        const aggregates = buildAggregates(perRun);
        const payload = {
            generatedAt: new Date().toISOString(),
            mode: useMock ? "mock" : "playwright",
            runs,
            url: targetUrl,
            environment,
            metrics: {
                perRun,
            },
            aggregates,
        };

        fs.writeFileSync(jsonPath, JSON.stringify(payload, null, 2));
        fs.writeFileSync(mdPath, renderMarkdown(payload));

        console.log(`Wrote benchmark results to:\n- ${jsonPath}\n- ${mdPath}`);
    } finally {
        if (serverProcess) {
            serverProcess.kill();
        }
    }
}

main().catch((error) => {
    console.error("Benchmark run failed", error);
    process.exit(1);
});

