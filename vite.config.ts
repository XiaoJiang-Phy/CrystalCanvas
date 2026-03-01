// [Overview: Vite frontend build configuration, optimized for Tauri with specific port settings]
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: true,
    port: 5173,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  // CRITICAL: @tauri-apps/api causes Vite's esbuild dep scanner to hang
  // forever in browser-only mode because it references __TAURI_INTERNALS__.
  // noDiscovery: true disables the automatic import graph crawl entirely.
  // We explicitly include only the deps we need pre-bundled.
  optimizeDeps: {
    noDiscovery: true,
    include: [
      "react",
      "react-dom",
      "react-dom/client",
      "react/jsx-runtime",
      "react/jsx-dev-runtime",
      "clsx",
      "tailwind-merge",
    ],
  },
});
