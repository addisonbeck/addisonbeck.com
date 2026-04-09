import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import pagefind from "astro-pagefind";
import { resolve } from "path";

export default defineConfig({
  output: "static",
  integrations: [svelte(), pagefind()],
  outDir: "./dist",
  server: {
    allowedHosts: true,
  },
  vite: {
    plugins: [
      {
        name: "pagefind-dev-shim",
        apply: "serve",
        resolveId(id) {
          if (id.startsWith("/pagefind/")) return id;
        },
        load(id) {
          if (id === "/pagefind/pagefind-ui.js") {
            return 'window.PagefindUI = class { constructor() { console.warn("Pagefind not available in dev mode — run `just build` first to generate the search index."); } };';
          }
          if (id === "/pagefind/pagefind-ui.css") {
            return "";
          }
        },
      },
      {
        name: "rendered-watcher",
        apply: "serve",
        configureServer(server) {
          server.watcher.add(resolve(process.cwd(), "../rendered"));
          server.watcher.on("change", (file) => {
            if (file.includes("/rendered/")) {
              server.ws.send({ type: "full-reload" });
            }
          });
        },
      },
    ],
    build: {
      rollupOptions: {
        external: ["/pagefind/pagefind-ui.js"],
      },
    },
  },
});
