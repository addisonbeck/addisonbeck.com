<script lang="ts">
  export let pages: Array<{ url: string; width: number; height: number }>;

  let currentPage = 0;

  $: pagesToPreload = new Set([
    currentPage,
    currentPage + 1,
    currentPage - 1,
  ]);

  function goTo(index: number) {
    currentPage = index;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      goTo(Math.min(currentPage + 1, pages.length - 1));
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      goTo(Math.max(currentPage - 1, 0));
    } else if (event.key === 'Home') {
      event.preventDefault();
      goTo(0);
    } else if (event.key === 'End') {
      event.preventDefault();
      goTo(pages.length - 1);
    }
  }
</script>

<section class="pdf-gallery" aria-roledescription="carousel" aria-label="PDF Document Pages" on:keydown={handleKeydown}>
  <div class="pdf-gallery__page" aria-live="polite" aria-atomic="false">
    {#each pages as page, i}
      <div
        role="region"
        aria-label={`Page ${i + 1} of ${pages.length}`}
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
  <div class="pdf-gallery__nav">
    <button
      class="pdf-gallery__btn"
      aria-label="Previous page"
      disabled={currentPage === 0}
      on:click={() => goTo(currentPage - 1)}
    >←</button>
    <span class="pdf-gallery__counter" aria-live="polite" aria-atomic="true">
      {currentPage + 1} / {pages.length}
    </span>
    <button
      class="pdf-gallery__btn"
      aria-label="Next page"
      disabled={currentPage === pages.length - 1}
      on:click={() => goTo(currentPage + 1)}
    >→</button>
  </div>
</section>
