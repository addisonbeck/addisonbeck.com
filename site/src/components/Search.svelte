<script lang="ts">
  import { onMount } from 'svelte';

  let searchEl: HTMLElement;

  onMount(async () => {
    await import('/pagefind/pagefind-ui.js');
    // pagefind-ui.js is an IIFE that attaches PagefindUI to window
    new (window as any).PagefindUI({
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
