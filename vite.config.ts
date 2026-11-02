import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// Standard Tauri v2 + Vite setup: the app root is src/, built into dist/.
export default defineConfig({
  root: "src",
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "safari16",
    // Two pages: the gauge window and the schedule editor window
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("src/index.html", import.meta.url)),
        editor: fileURLToPath(new URL("src/editor.html", import.meta.url)),
      },
    },
  },
  test: {
    root: ".",
    include: ["test/**/*.test.ts"],
  },
});
