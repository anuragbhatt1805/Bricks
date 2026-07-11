<script lang="ts">
  import { X, Check, Square } from "lucide-svelte";
  export let visible = false;
  export let backendName = "No backend";
  export let local = true;
  let width = 360;
  let message = "";
</script>

{#if visible}
  <aside class="agent" style={`width:${width}px`}>
    <header>
      <strong>Agent</strong>
      <span class="backend">{backendName} {local ? "local" : "remote"}</span>
    </header>
    <section class="transcript">
      <div class="assistant">Agentic mode is ready. Backends and execution loop are guarded by the Rust Normal-mode check.</div>
      <div class="proposal">
        <span class="risk">Confirm</span>
        <code>npm install lodash</code>
        <footer>
          <button><Check size={14} />Approve</button>
          <button>Edit</button>
          <button><X size={14} />Reject</button>
        </footer>
      </div>
      <div class="blocked">
        <strong>Blocked</strong>
        <code>rm -rf /</code>
      </div>
    </section>
    <footer>
      <textarea bind:value={message} rows="3" placeholder="Ask Brick"></textarea>
      <button><Square size={14} />Cancel</button>
    </footer>
  </aside>
{/if}
