<script lang="ts">
  export let pages: Array<{ url: string; width: number; height: number }>;

  let currentPage = 0;
  let tabEls: HTMLButtonElement[] = [];

  $: pagesToPreload = new Set([
    currentPage,
    currentPage + 1,
    currentPage - 1,
  ]);

  function goTo(index: number) {
    currentPage = index;
    tabEls[currentPage]?.focus();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      goTo((currentPage + 1) % pages.length);
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      goTo((currentPage - 1 + pages.length) % pages.length);
    } else if (event.key === 'Home') {
      event.preventDefault();
      goTo(0);
    } else if (event.key === 'End') {
      event.preventDefault();
      goTo(pages.length - 1);
    }
  }
</script>

<section
  role="region"
  aria-roledescription="carousel"
  aria-label="PDF Document Pages"
>
  <div
    role="tablist"
    aria-label="PDF pages"
    on:keydown={handleKeydown}
  >
    {#each pages as _page, i}
      <button
        role="tab"
        aria-selected={i === currentPage}
        aria-controls={`pdf-panel-${i}`}
        id={`pdf-tab-${i}`}
        tabindex={i === currentPage ? 0 : -1}
        bind:this={tabEls[i]}
        on:click={() => goTo(i)}
      >
        {i + 1}
      </button>
    {/each}
  </div>

  <div aria-live="polite" aria-atomic="false">
    {#each pages as page, i}
      <div
        role="tabpanel"
        id={`pdf-panel-${i}`}
        aria-labelledby={`pdf-tab-${i}`}
        hidden={i !== currentPage}
      >
        {#if pagesToPreload.has(i)}
          <img
            src={page.url}
            alt={`Page ${i + 1} of ${pages.length}`}
            width={page.width}
            height={page.height}
          />
        {/if}
      </div>
    {/each}
  </div>
</section>
