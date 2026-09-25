import { createApp } from "vue";
import App from "./App.vue";
import "./ui.css";

// The UI is useless without the Tauri IPC bridge: opening the Vite URL or
// dist/index.html in a plain browser used to fail with
// "Cannot read properties of undefined (reading 'invoke')". Say so instead.
if ("__TAURI_INTERNALS__" in window) {
  createApp(App).mount("#app");
} else {
  const node = document.querySelector<HTMLElement>("#app");
  if (node) {
    node.style.cssText =
      "padding:24px;font:13px/1.6 'Segoe UI',system-ui,sans-serif;color:#dfe1e6;background:#16181c;height:100vh";
    node.textContent =
      "AreTwo - R2 Explorer needs its desktop shell for the R2 connection. Start it with `pnpm tauri dev`, " +
      "or run the installed aretwo.exe. A plain browser tab cannot talk to R2.";
  }
}
