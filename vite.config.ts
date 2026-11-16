import { defineConfig } from "vite";
import dyadComponentTagger from "@dyad-sh/react-vite-component-tagger";
import react from "@vitejs/plugin-react-swc";
import path from "path";

export default defineConfig(({ command }) => ({
  base: "./",
  server: {
    host: "::",
    port: 8080,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/src/native/**"] },
  },
  plugins: [react(), ...(command === "serve" && !process.env.TAURI_ENV_PLATFORM ? [dyadComponentTagger()] : [])],
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  build: { target: "es2020", sourcemap: false },
  clearScreen: false,
}));
