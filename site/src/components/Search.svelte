<script lang="ts">
  import { onMount } from 'svelte';

  let searchEl: HTMLElement;

  onMount(async () => {
    // @ts-ignore — pagefind loaded at runtime from dist/pagefind/
    const { PagefindUI } = await import('/pagefind/pagefind-ui.js');
    new PagefindUI({
      element: searchEl,
      showSubResults: true,
    });
  });
</script>

<div bind:this={searchEl} class="search-widget" />

<link rel="stylesheet" href="/pagefind/pagefind-ui.css" />

<style>
  .search-widget {
    max-width: 600px;
    margin: 0 auto;
  }
</style>
