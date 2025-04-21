import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  build: {
    rollupOptions: {
      output: {
        format: "iife",
        dir: "dist/",
      },
    },
  },
  ssr: {
    target: "webworker",
    noExternal: true,
  },
  plugins: [react()],
});
