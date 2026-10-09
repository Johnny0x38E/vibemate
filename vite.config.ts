import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  // Keep Rust build errors visible while Vite updates the frontend.
  clearScreen: false,
  server: {
    // Tauri's devUrl uses this port, so a silent port change would break startup.
    port: 1420,
    strictPort: true,
    host: host || false,
    // Omit optional settings entirely when absent; do not assign undefined.
    ...(host ? { hmr: { protocol: "ws", host, port: 1421 } } : {}),
    // Rust rebuilds are handled by Tauri rather than the frontend file watcher.
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
