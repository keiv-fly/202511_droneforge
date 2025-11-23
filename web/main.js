import init from "./wasm/droneforge.js";

async function run() {
  await init();
}

run().catch((err) => {
  console.error("Failed to start Droneforge WASM:", err);
});
