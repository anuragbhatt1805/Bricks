<script lang="ts">
  import { Check, X } from "lucide-svelte";
  let tab: "Backends" | "Shell Integration" | "Data" | "Appearance" = "Shell Integration";
  const tabs = ["Backends", "Shell Integration", "Data", "Appearance"] as const;
</script>

<section class="settings">
  <nav>
    {#each tabs as item}
      <button class:active={tab === item} on:click={() => (tab = item)}>{item}</button>
    {/each}
  </nav>
  {#if tab === "Shell Integration"}
    <div class="settings-pane">
      <h2>zsh</h2>
      <p><X size={14} /> Not installed</p>
      <button>Install</button>
      <button>Uninstall</button>
    </div>
  {:else if tab === "Backends"}
    <div class="settings-pane">
      <h2>Backends</h2>
      <input placeholder="Ollama" />
      <input placeholder="http://localhost:11434" />
      <input placeholder="llama3.1" />
      <button>Test connection</button>
    </div>
  {:else if tab === "Data"}
    <div class="settings-pane">
      <p><Check size={14} /> Local database: ~/Library/Application Support/dev.brick.app/brick.db</p>
      <button>Purge now</button>
      <button>Export JSON</button>
    </div>
  {:else}
    <div class="settings-pane">
      <label>Font size <input type="range" min="10" max="22" value="13" /></label>
      <label><input type="checkbox" /> Dark</label>
    </div>
  {/if}
</section>
