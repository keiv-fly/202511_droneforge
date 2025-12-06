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

window.addEventListener("load", () => {
    const startScreen = document.getElementById("start-screen");
    const startButton = document.getElementById("start-button");

    startButton.addEventListener("click", async () => {
        startButton.disabled = true;
        await requestFullscreenIfMobile();
        startScreen.style.display = "none";
    });
});

