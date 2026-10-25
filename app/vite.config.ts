import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri loads the dev server at this fixed port (src-tauri/tauri.conf.json, devUrl).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: ["es2022", "safari16"],
  },
});
