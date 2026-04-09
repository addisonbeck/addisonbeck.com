<script>
  import { onMount } from "svelte";

  export let slug;
  export let title;
  export let variant = "sidebar"; // "sidebar" | "nav"

  const STORAGE_KEY = "recently-viewed";
  const MAX_ITEMS = 5;

  let recentItems = [];

  onMount(() => {
    let stored = [];
    try {
      stored = JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
    } catch {
      stored = [];
    }

    const next = [
      { slug, title },
      ...stored.filter((item) => item.slug !== slug),
    ].slice(0, MAX_ITEMS + 1);

    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {}

    recentItems = next.filter((item) => item.slug !== slug);
  });
</script>

{#if recentItems.length > 0}
  {#if variant === "sidebar"}
    <aside class="rv-sidebar" aria-label="Recently viewed">
      <span class="rv-label">Recent</span>
      <ul>
        {#each recentItems as item (item.slug)}
          <li><a href="/{item.slug}">{item.title}</a></li>
        {/each}
      </ul>
    </aside>
  {:else}
    <details class="rv-nav">
      <summary>Recent</summary>
      <ul>
        {#each recentItems as item (item.slug)}
          <li><a href="/{item.slug}">{item.title}</a></li>
        {/each}
      </ul>
    </details>
  {/if}
{/if}

<style>
  /* Desktop sidebar (column 3) */
  .rv-sidebar {
    display: none;
    padding-left: 1rem;
    border-left: 1px solid var(--bg3);
  }

  @media (min-width: 900px) {
    .rv-sidebar {
      display: block;
    }
    .rv-nav {
      display: none;
    }
  }

  .rv-label {
    display: block;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fg4);
    margin-bottom: 0.5rem;
  }

  .rv-sidebar ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .rv-sidebar li {
    margin-bottom: 0.4rem;
  }

  .rv-sidebar a {
    color: var(--fg4);
    font-size: 0.8rem;
    text-decoration: underline;
    line-height: 1.3;
    display: block;
  }

  .rv-sidebar a:hover {
    color: var(--fg2);
  }

  /* Mobile nav dropdown */
  .rv-nav {
    position: relative;
  }

  .rv-nav summary {
    cursor: pointer;
    list-style: none;
    color: var(--fg4);
    font-size: 0.875rem;
    user-select: none;
  }

  .rv-nav summary::-webkit-details-marker {
    display: none;
  }

  .rv-nav ul {
    list-style: none;
    padding: 0.5rem;
    margin: 0;
    width: 80vw;
    position: absolute;
    top: calc(100% + 0.4rem);
    right: 0;
    background: var(--bg1);
    border: 1px solid var(--bg3);
    border-radius: 3px;
    z-index: 10;
  }

  .rv-nav li {
    margin-bottom: 0.25rem;
  }

  .rv-nav a {
    color: var(--fg3);
    font-size: 0.8rem;
    text-decoration: underline;
  }

  .rv-nav a:hover {
    color: var(--fg);
  }
</style>
