<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { Bot, PanelRight, Settings as SettingsIcon, Terminal } from "lucide-svelte";
  import AgentPanel from "./lib/AgentPanel.svelte";
  import PromptInfoBar from "./lib/PromptInfoBar.svelte";
  import Settings from "./lib/Settings.svelte";
  import XTerm from "./lib/XTerm.svelte";
  import type { WorkspaceMode } from "./lib/types";

  let paneId = "";
  let cwd = "/Users/anurag/Workspace/brick";
  let mode: WorkspaceMode = "normal";
  let showAgent = false;
  let showSettings = false;
  let termRef: XTerm;

  async function boot() {
    paneId = await invoke<string>("spawn_shell", {
      shellPath: "/bin/zsh",
      cwd: cwd,
    });
  }


  // Handle direct terminal typing
  function handleTermInput(data: string) {
    if (!paneId) return;
    invoke("pty_write", { paneId, data: Array.from(new TextEncoder().encode(data)) });
  }

  // Handle terminal resizing
  function handleTermResize(cols: number, rows: number) {
    if (!paneId) return;
    invoke("pty_resize", { paneId, cols, rows });
  }

  async function toggleMode() {
    mode = mode === "normal" ? "agentic" : "normal";
    showAgent = mode === "agentic";
    await invoke("set_pane_mode", { paneId, mode });
  }

  listen<{ pane_id: string; text: string }>("pty_output", (event) => {
    if (event.payload.pane_id !== paneId) return;
    if (termRef) {
      termRef.write(event.payload.text);
    }
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
      
      <div class="mode-toggle-group">
        <button 
          class:agentic={mode === "agentic"} 
          class="mode-btn" 
          on:click={toggleMode}
          title="Toggle between Normal shell and AI-Assisted Agentic shell modes"
        >
          {mode === "agentic" ? "⚡ Agentic" : "🖥 Normal"}
        </button>
      </div>

      <div class="topbar-actions">
        <button 
          title="Toggle AI Agent Panel" 
          class:active={showAgent}
          on:click={() => (showAgent = !showAgent)}
        >
          <PanelRight size={16} />
        </button>
        <button 
          title="Configure Settings & LLM Backends" 
          class:active={showSettings}
          on:click={() => (showSettings = !showSettings)}
        >
          <SettingsIcon size={16} />
        </button>
      </div>
    </header>

    <PromptInfoBar {cwd} />

    {#if showSettings}
      <Settings />
    {:else}
      <div class="terminal-pane">
        <XTerm 
          bind:this={termRef} 
          onInput={handleTermInput} 
          onResize={handleTermResize} 
        />
      </div>
    {/if}

    <!-- Status Bar for Discoverability -->
    <footer class="statusbar">
      <div class="status-item">
        <span class="dot" class:green={mode === "agentic"}></span>
        Mode: {mode === "agentic" ? "Agentic (AI active)" : "Normal (Standard shell)"}
      </div>
      <div class="status-item font-mono">CWD: {cwd}</div>
    </footer>
  </section>

  <AgentPanel 
    visible={showAgent} 
    {mode}
    {paneId} 
    onEnableAgentic={toggleMode}
  />
</main>

<style>
  main {
    display: flex;
    width: 100vw;
    height: 100vh;
    background-color: #1e1e1e;
    overflow: hidden;
  }
  .workspace {
    flex: 1;
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .topbar {
    display: flex;
    align-items: center;
    background-color: #252526;
    border-bottom: 1px solid #2d2d2d;
    padding: 8px 16px;
    height: 48px;
    box-sizing: border-box;
    gap: 16px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #ffffff;
    font-weight: 600;
    font-size: 14px;
  }
  .mode-toggle-group {
    margin-left: auto;
  }
  .mode-btn {
    background-color: #37373d;
    color: #cccccc;
    border: 1px solid #47474f;
    padding: 6px 14px;
    border-radius: 4px;
    font-size: 12px;
    cursor: pointer;
    font-weight: bold;
    transition: all 0.2s ease;
  }
  .mode-btn.agentic {
    background-color: #007acc;
    color: #ffffff;
    border-color: #0098ff;
    box-shadow: 0 0 8px rgba(0, 122, 204, 0.4);
  }
  .topbar-actions {
    display: flex;
    gap: 8px;
  }
  .topbar-actions button {
    background: none;
    border: none;
    color: #858585;
    padding: 6px;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .topbar-actions button:hover, .topbar-actions button.active {
    color: #ffffff;
    background-color: #37373d;
  }
  .terminal-pane {
    flex: 1;
    background-color: #1e1e1e;
    position: relative;
    overflow: hidden;
  }
  .statusbar {
    display: flex;
    background-color: #007acc;
    color: #ffffff;
    padding: 4px 12px;
    font-size: 11px;
    justify-content: space-between;
    align-items: center;
    box-sizing: border-box;
    height: 22px;
  }
  .status-item {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .font-mono {
    font-family: monospace;
  }
  .dot {
    width: 6px;
    height: 6px;
    background-color: #858585;
    border-radius: 50%;
  }
  .dot.green {
    background-color: #0dbc79;
    box-shadow: 0 0 4px #0dbc79;
  }
</style>
