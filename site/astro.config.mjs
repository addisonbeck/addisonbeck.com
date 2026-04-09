import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import pagefind from "astro-pagefind";
import { resolve, extname } from "path";
import { createReadStream, existsSync } from "fs";

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
        // Serve pagefind assets from dist/pagefind/ in dev mode.
        // pagefind.js is imported with /* @vite-ignore */ so Vite skips module
        // graph analysis for it — the browser fetches it directly via HTTP here.
        name: "pagefind-dev-shim",
        apply: "serve",
        configureServer(server) {
          server.middlewares.use("/pagefind", (req, res, next) => {
            const file = req.url.split("?")[0].replace(/^\//, "");
            const filePath = resolve(process.cwd(), "dist/pagefind", file);
            if (existsSync(filePath)) {
              const mime =
                {
                  ".js": "application/javascript",
                  ".css": "text/css",
                  ".wasm": "application/wasm",
                  ".json": "application/json",
                }[extname(filePath)] ?? "application/octet-stream";
              res.setHeader("Content-Type", mime);
              createReadStream(filePath).pipe(res);
            } else {
              next();
            }
          });
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
  },
});
