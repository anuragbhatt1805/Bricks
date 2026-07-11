<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { SuggestedCommand } from "./types";

  export let cwd = "/";
  export let disabled = false;
  export let onSubmit: (command: string) => void = () => {};

  let value = "";
  let suggestions: SuggestedCommand[] = [];
  let active = 0;
  let timer: number | undefined;

  $: top = suggestions[active]?.command ?? "";
  $: suffix = top.startsWith(value) && value.length > 0 ? top.slice(value.length) : "";

  function refresh() {
    clearTimeout(timer);
    timer = window.setTimeout(async () => {
      suggestions = await invoke<SuggestedCommand[]>("suggest_command", { partial: value, cwd });
      active = 0;
    }, 50);
  }

  function keydown(event: KeyboardEvent) {
    if (event.key === "Tab" && top) {
      event.preventDefault();
      value = top;
      suggestions = [];
    } else if (event.key === "Escape") {
      suggestions = [];
    } else if (event.key === "ArrowDown" && suggestions.length) {
      event.preventDefault();
      active = (active + 1) % suggestions.length;
    } else if (event.key === "Enter") {
      event.preventDefault();
      onSubmit(value);
      value = "";
      suggestions = [];
    }
  }
</script>

<div class="command-input">
  <div class="ghost"><span>{value}</span><em>{suffix}</em></div>
  <input bind:value disabled={disabled} on:input={refresh} on:keydown={keydown} spellcheck="false" />
  {#if suggestions.length > 1}
    <div class="suggestions">
      {#each suggestions as suggestion, i}
        <button class:active={i === active} on:mousedown={() => (value = suggestion.command)}>
          {suggestion.command}
        </button>
      {/each}
    </div>
  {/if}
</div>
