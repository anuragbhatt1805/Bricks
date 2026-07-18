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
  <span class="prompt-symbol">$</span>
  <div class="input-wrapper">
    <div class="ghost"><span>{value}</span><em>{suffix}</em></div>
    <input 
      bind:value 
      disabled={disabled} 
      on:input={refresh} 
      on:keydown={keydown} 
      spellcheck="false" 
      placeholder="Type a command and press Enter..." 
    />
  </div>
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

<style>
  .command-input {
    display: flex;
    align-items: center;
    background-color: #1e1e1e;
    border-top: 1px solid #2d2d2d;
    padding: 8px 12px;
    position: relative;
    gap: 8px;
  }
  .prompt-symbol {
    color: #0dbc79;
    font-family: monospace;
    font-weight: bold;
    font-size: 14px;
    user-select: none;
  }
  .input-wrapper {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
  }
  input {
    width: 100%;
    background: transparent;
    border: none;
    color: #ffffff;
    font-family: monospace;
    font-size: 13px;
    padding: 4px 0;
    outline: none;
  }
  input::placeholder {
    color: #555555;
  }
  .ghost {
    position: absolute;
    left: 0;
    top: 4px;
    font-family: monospace;
    font-size: 13px;
    color: #555555;
    pointer-events: none;
    white-space: pre;
  }
  .ghost span {
    visibility: hidden;
  }
  .suggestions {
    position: absolute;
    bottom: 100%;
    left: 28px;
    right: 12px;
    background-color: #252526;
    border: 1px solid #3c3c3c;
    border-radius: 4px;
    box-shadow: 0 -4px 12px rgba(0, 0, 0, 0.5);
    z-index: 100;
    max-height: 200px;
    overflow-y: auto;
  }
  .suggestions button {
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    color: #cccccc;
    padding: 6px 12px;
    font-family: monospace;
    font-size: 12px;
    cursor: pointer;
  }
  .suggestions button.active, .suggestions button:hover {
    background-color: #37373d;
    color: #ffffff;
  }
</style>
