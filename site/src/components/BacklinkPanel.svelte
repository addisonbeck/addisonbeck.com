<script lang="ts">
  interface Backlink {
    id: string;
    title: string;
    slug: string;
  }

  export let backlinks: Backlink[] = [];
</script>

{#if backlinks.length > 0}
  <section class="backlinks">
    <h2>Linked from</h2>
    <ul>
      {#each backlinks as link}
        <li>
          <a href="/{link.slug}">{link.title}</a>
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  .backlinks {
    margin-top: 3rem;
    padding-top: 1rem;
    border-top: 1px solid #ccc;
  }
  .backlinks h2 {
    font-size: 0.875rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #666;
  }
  .backlinks ul {
    list-style: none;
    padding: 0;
  }
  .backlinks li {
    margin-bottom: 0.25rem;
  }
</style>
