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
  },
  test: {
    root: ".",
    include: ["test/**/*.test.ts"],
  },
});
