import { defineConfig, loadEnv, type UserConfig } from "vite";
import react from "@vitejs/plugin-react";
import mkcert from "vite-plugin-mkcert";

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");

  // Only require VITE_API_BASE when actually deploying to Vercel (not local builds)
  if (mode === "production" && !env.VITE_API_BASE && process.env.VERCEL) {
    throw new Error(
      "Missing VITE_API_BASE environment variable for production builds. Configure it in your Vercel project settings.",
    );
  }

  const config: UserConfig = {
    plugins: [react()],
    build: {
      sourcemap: false,
      chunkSizeWarningLimit: 1000,
      rollupOptions: {
        output: {
          manualChunks(id) {
            if (id.includes("node_modules/monaco-editor") || id.includes("@monaco-editor")) {
              return "monaco";
            }
            if (id.includes("node_modules/qrcode")) {
              return "qr";
            }
            if (
              id.includes("node_modules/react-dom") ||
              id.includes("node_modules/react/") ||
              id.includes("node_modules/react-router")
            ) {
              return "react";
            }
          },
        },
      },
    },
  };

  if (mode === "development") {
    config.plugins?.push(mkcert());
    config.server = {
      host: "127.0.0.1",
      port: 5173,
      proxy: {
        "/api": {
          target: "http://127.0.0.1:8000",
          changeOrigin: true,
        },
        "/raw": {
          target: "http://127.0.0.1:8000",
          changeOrigin: true,
        },
      },
    };
  }

  return config;
});
