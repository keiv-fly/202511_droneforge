function forwardZChange(callbackName) {
    const handler = wasm_exports?.[callbackName];
    if (typeof handler === "function") {
        handler();
    }
}

window.addEventListener("load", () => {
    const zUp = document.getElementById("z-up");
    const zDown = document.getElementById("z-down");
    const canvas = document.getElementById("glcanvas");

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
});

