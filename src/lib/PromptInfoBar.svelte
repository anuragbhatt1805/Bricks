<script lang="ts">
  export let cwd = "/";
  export let branch: string | null = null;
  export let dirty = false;
  export let nodeVersion: string | null = null;

  $: parts = cwd.split("/").filter(Boolean);
  $: shortCwd = parts.length > 3 ? `.../${parts.slice(-3).join("/")}` : cwd;
</script>

<div class="prompt-info">
  <button class="path" title={cwd}>{shortCwd}</button>
  {#if branch}
    <span class="badge">⎇ {branch}{dirty ? " •" : ""}</span>
  {/if}
  {#if nodeVersion}
    <span class="badge">Node {nodeVersion}</span>
  {/if}
</div>
