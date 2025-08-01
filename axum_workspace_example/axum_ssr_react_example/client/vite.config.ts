import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react-swc'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "dist/client",
    rollupOptions: {
      input: "./src/main.tsx",
      output: {
        entryFileNames: "index.js",
        assetFileNames: "assets/[name][extname]",
      },
    },
  },
})
