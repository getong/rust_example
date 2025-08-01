import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";

// https://vitejs.dev/config/
export default defineConfig({
  build: {
    ssr: true,
    outDir: "dist/ssr",
    emptyOutDir: true,
    rollupOptions: {
      input: "./src/server-entry.tsx",
      output: {
        format: "iife",
        entryFileNames: "index.js",
        name: "SSR",
      },
    },
  },
  ssr: {
    target: "webworker",
    noExternal: true,
  },
  plugins: [react()],
});
