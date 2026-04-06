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
});
