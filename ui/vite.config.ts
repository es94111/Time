import { defineConfig } from "vite";

// Tauri 期望固定埠與相對資源路徑（以自訂協定載入）。
export default defineConfig({
  base: "./",
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "esnext",
  },
});
