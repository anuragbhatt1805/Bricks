<script lang="ts">
  import { Copy, Play, MessageSquare, Check, X } from "lucide-svelte";
  import type { Block } from "./types";

  export let block: Block;
  let expanded = false;

  $: lines = block.output.split("\n");
  $: collapsed = lines.length > 50 && !expanded;
  $: visibleOutput = collapsed ? lines.slice(0, 50).join("\n") : block.output;
</script>

<article class:interactive={block.is_interactive} class="block">
  <header>
    <code>{block.command}</code>
    <span class="cwd">{block.cwd}</span>
    <span class="spacer"></span>
    {#if block.exit_code === 0}
      <span class="exit ok"><Check size={14} /></span>
    {:else if block.exit_code !== null}
      <span class="exit bad"><X size={14} /> {block.exit_code}</span>
    {/if}
    <span class="duration">{block.duration_ms}ms</span>
    <nav>
      <button title="Copy command" on:click={() => navigator.clipboard.writeText(block.command)}><Copy size={14} /></button>
      <button title="Re-run"><Play size={14} /></button>
      <button title="Ask agent"><MessageSquare size={14} /></button>
    </nav>
  </header>

  {#if block.is_interactive}
    <p class="compact">Interactive session</p>
  {:else}
    <pre>{visibleOutput}</pre>
    {#if collapsed}
      <button class="more" on:click={() => (expanded = true)}>Show more</button>
    {/if}
  {/if}
</article>
