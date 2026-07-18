<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Bot, PanelRight, Settings as SettingsIcon, Terminal } from "lucide-svelte";
  import AgentPanel from "./lib/AgentPanel.svelte";
  import BlockList from "./lib/BlockList.svelte";
  import CommandInput from "./lib/CommandInput.svelte";
  import PromptInfoBar from "./lib/PromptInfoBar.svelte";
  import Settings from "./lib/Settings.svelte";
  import type { Block, WorkspaceMode } from "./lib/types";

  let paneId = "";
  let cwd = "/";
  let mode: WorkspaceMode = "normal";
  let rawOutput = "";
  let currentInput = "";
  let showAgent = false;
  let showSettings = false;
  let blocks: Block[] = [];

  async function boot() {
    cwd = await navigator.storage?.getDirectory?.().then(() => cwd).catch(() => cwd) ?? cwd;
    paneId = await invoke<string>("spawn_shell", {
      shellPath: "/bin/zsh",
      cwd: "/Users/anurag/Workspace/brick",
    });
  }

  function submit(command: string) {
    if (!paneId || !command.trim()) return;
    invoke("pty_write", { paneId, data: Array.from(new TextEncoder().encode(`${command}\n`)) });
    currentInput = command;
  }

  async function toggleMode() {
    mode = mode === "normal" ? "agentic" : "normal";
    showAgent = mode === "agentic";
    await invoke("set_pane_mode", { paneId, mode });
  }

  listen<{ pane_id: string; text: string }>("pty_output", (event) => {
    if (event.payload.pane_id !== paneId) return;
    rawOutput += event.payload.text;
    if (rawOutput.length > 100_000) rawOutput = rawOutput.slice(-100_000);
  });

  listen<{ block_id: string; pane_id: string }>("block_finished", (event) => {
    if (event.payload.pane_id !== paneId) return;
    blocks = [
      {
        id: event.payload.block_id,
        command: currentInput || "(shell command)",
        cwd,
        exit_code: 0,
        duration_ms: 0,
        output: rawOutput.split("\n").slice(-80).join("\n"),
      },
      ...blocks,
    ];
    currentInput = "";
  });

  listen("agent_panel_hidden", () => {
    showAgent = false;
    mode = "normal";
  });

  boot();
</script>

<main>
  <section class="workspace">
    <header class="topbar">
      <div class="brand"><Terminal size={18} /> Brick</div>
      <button class:agentic={mode === "agentic"} class="mode" on:click={toggleMode}>
        {mode === "agentic" ? "Agentic ⚡" : "Normal"}
      </button>
      <button title="Agent panel" on:click={() => (showAgent = !showAgent)}><PanelRight size={16} /></button>
      <button title="Settings" on:click={() => (showSettings = !showSettings)}><SettingsIcon size={16} /></button>
    </header>

    <PromptInfoBar {cwd} />

    {#if showSettings}
      <Settings />
    {:else}
      <div class="terminal-pane">
        <pre class="raw">{rawOutput || "Starting zsh..."}</pre>
        <BlockList {blocks} />
      </div>
      <CommandInput {cwd} onSubmit={submit} />
    {/if}
  </section>

  <AgentPanel visible={showAgent && mode === "agentic"} {paneId} />
</main>
