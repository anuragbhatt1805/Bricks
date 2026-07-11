<script lang="ts">
  import BlockView from "./Block.svelte";
  import type { Block } from "./types";

  export let blocks: Block[] = [];
  let viewport: HTMLDivElement;
  let scrollTop = 0;
  const rowHeight = 148;

  $: height = viewport?.clientHeight ?? 600;
  $: start = Math.max(0, Math.floor(scrollTop / rowHeight) - Math.ceil(height / rowHeight));
  $: end = Math.min(blocks.length, start + Math.ceil((height * 3) / rowHeight) + 4);
  $: visible = blocks.slice(start, end);
</script>

<div
  class="block-list"
  bind:this={viewport}
  on:scroll={() => {
    scrollTop = viewport.scrollTop;
  }}
>
  <div class="spacer" style={`height: ${blocks.length * rowHeight}px`}>
    <div class="window" style={`transform: translateY(${start * rowHeight}px)`}>
      {#each visible as block (block.id)}
        <BlockView {block} />
      {/each}
    </div>
  </div>
</div>
