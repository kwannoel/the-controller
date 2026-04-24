import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { execSync } from "child_process";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// @ts-expect-error process is a nodejs global
const host = process.env.DEV_HOST;
// @ts-expect-error process is a nodejs global
const port = parseInt(process.env.DEV_PORT || "1420", 10);
// @ts-expect-error process is a nodejs global
const axumPort = process.env.AXUM_PORT || "3001";

function git(cmd: string): string {
  try {
    return execSync(`git ${cmd}`, { encoding: "utf-8" }).trim();
  } catch {
    return "unknown";
  }
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [svelte()],

  resolve: {
    alias: {
      "$lib": path.resolve(__dirname, "src/lib"),
    },
  },

  define: {
    __BUILD_COMMIT__: JSON.stringify(git("rev-parse --short HEAD")),
    __BUILD_BRANCH__: JSON.stringify(git("rev-parse --abbrev-ref HEAD")),
    __DEV_PORT__: JSON.stringify(port),
  },

  clearScreen: false,
  server: {
    port,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: port + 1,
        }
      : undefined,
    watch: {
      ignored: ["**/server/**"],
    },
    proxy: {
      "/api": `http://localhost:${axumPort}`,
      "/ws": {
        target: `ws://localhost:${axumPort}`,
        ws: true,
      },
    },
  },
}));
