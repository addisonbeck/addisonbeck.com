import { defineConfig } from 'astro/config';
import svelte from '@astrojs/svelte';
import pagefind from 'astro-pagefind';

export default defineConfig({
  // Static output: no server-side rendering
  output: 'static',
  integrations: [
    svelte(),
    pagefind(),
  ],
  // Build output directory
  outDir: './dist',
  vite: {
    build: {
      rollupOptions: {
        // pagefind-ui.js is generated at build time by astro-pagefind — not resolvable at bundle time
        external: ['/pagefind/pagefind-ui.js'],
      },
    },
  },
});
