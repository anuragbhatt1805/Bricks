<script lang="ts">
  import { X, Check, Square, Play, Trash2, ShieldAlert } from "lucide-svelte";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  export let visible = false;
  export let mode: "normal" | "agentic" = "normal";
  export let paneId = "";
  export let onEnableAgentic: () => void = () => {};

  let width = 360;
  let message = "";
  let isStreaming = false;
  let contextTrimmed = false;

  interface Backend {
    id: string;
    name: string;
    kind: string;
    is_default: boolean;
    is_local: boolean;
  }
  let backends: Backend[] = [];
  let activeBackendId = "";

  interface TranscriptItem {
    type: "user" | "assistant" | "proposal" | "blocked" | "system";
    text?: string;
    command?: string;
    riskTier?: string;
    reason?: string;
    done?: boolean;
    contextTrimmed?: boolean;
  }
  let transcript: TranscriptItem[] = [];

  let editingIndex = -1;
  let editedCommand = "";

  async function loadWidth() {
    let saved = await invoke<string | null>("get_setting", { key: "agent.panel_width" });
    if (saved) {
      let w = parseInt(saved);
      if (w >= 280 && w <= 800) {
        width = w;
      }
    }
  }

  async function loadBackends() {
    try {
      backends = await invoke<Backend[]>("list_backends");
      let active = await invoke<string | null>("get_setting", { key: "agent.active_backend_id" });
      if (active) {
        activeBackendId = active;
      } else {
        let def = backends.find(b => b.is_default);
        if (def) {
          activeBackendId = def.id;
        } else if (backends.length > 0) {
          activeBackendId = backends[0].id;
        }
      }
    } catch (e) {
      console.error(e);
    }
  }

  async function handleBackendChange() {
    await invoke("set_setting", { key: "agent.active_backend_id", value: activeBackendId });
  }

  function startResize(e: MouseEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startWidth = width;

    function onMouseMove(moveEvent: MouseEvent) {
      const deltaX = startX - moveEvent.clientX; // drag left to make wider
      const newWidth = Math.max(280, Math.min(800, startWidth + deltaX));
      width = newWidth;
    }

    function onMouseUp() {
      invoke("set_setting", { key: "agent.panel_width", value: width.toString() });
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    }

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  }

  async function send() {
    if (!message.trim() || isStreaming) return;
    let text = message;
    message = "";
    transcript = [...transcript, { type: "user", text }];
    isStreaming = true;

    try {
      await invoke("agent_run_turn", {
        paneId,
        userMessage: text,
      });
    } catch (e: any) {
      transcript = [...transcript, {
        type: "assistant",
        text: `Error running turn: ${e.toString()}`,
      }];
      isStreaming = false;
    }
  }

  async function cancel() {
    try {
      await invoke("cancel_agent_turn", { paneId });
    } catch (e) {
      console.error(e);
    }
  }

  async function approve(idx: number, command: string) {
    transcript[idx].done = true;
    transcript = [...transcript];
    try {
      await invoke("agent_approve_command", { paneId, command });
    } catch (e) {
      console.error(e);
    }
  }

  function startEdit(idx: number, command: string) {
    editingIndex = idx;
    editedCommand = command;
  }

  async function saveEdit(idx: number) {
    let cmd = editedCommand;
    editingIndex = -1;
    transcript[idx].command = cmd;
    approve(idx, cmd);
  }

  async function reject(idx: number) {
    transcript[idx].done = true;
    transcript = [...transcript];
    try {
      await invoke("agent_reject_command", { paneId });
    } catch (e) {
      console.error(e);
    }
  }

  onMount(() => {
    let unmounted = false;
    let unlistens: (() => void)[] = [];

    async function init() {
      await loadWidth();
      await loadBackends();
      if (unmounted) return;

      let u1 = await listen<any>("agent_stream_chunk", (event) => {
        let payload = event.payload;
        if (payload.pane_id !== paneId) return;

        isStreaming = true;
        let lastIdx = transcript.length - 1;
        if (lastIdx >= 0 && transcript[lastIdx].type === "assistant" && !transcript[lastIdx].done) {
          transcript[lastIdx].text += payload.delta;
          transcript = [...transcript];
        } else {
          transcript = [...transcript, {
            type: "assistant",
            text: payload.delta,
            done: false,
          }];
        }
      });

      let u2 = await listen<any>("agent_proposed_command", (event) => {
        let payload = event.payload;
        if (payload.pane_id !== paneId) return;

        // Finish any ongoing streaming assistant box
        let lastIdx = transcript.length - 1;
        if (lastIdx >= 0 && transcript[lastIdx].type === "assistant") {
          transcript[lastIdx].done = true;
        }

        transcript = [...transcript, {
          type: "proposal",
          command: payload.command,
          riskTier: payload.risk,
          done: false,
        }];
        isStreaming = false;
      });

      let u3 = await listen<any>("agent_blocked_command", (event) => {
        let payload = event.payload;
        if (payload.pane_id !== paneId) return;

        let lastIdx = transcript.length - 1;
        if (lastIdx >= 0 && transcript[lastIdx].type === "assistant") {
          transcript[lastIdx].done = true;
        }

        transcript = [...transcript, {
          type: "blocked",
          command: payload.command,
          reason: payload.reason,
        }];
        isStreaming = false;
      });

      let u4 = await listen<any>("agent_cancelled", (event) => {
        let payload = event.payload;
        if (payload.pane_id !== paneId) return;
        isStreaming = false;
        let lastIdx = transcript.length - 1;
        if (lastIdx >= 0 && transcript[lastIdx].type === "assistant") {
          transcript[lastIdx].text += "\n[Cancelled by user]";
          transcript = [...transcript];
        }
      });

      let u5 = await listen<any>("agent_turn_done", (event) => {
        let payload = event.payload;
        if (payload.pane_id !== paneId) return;
        isStreaming = false;
      });

      unlistens = [u1, u2, u3, u4, u5];
    }

    init();

    return () => {
      unmounted = true;
      for (let u of unlistens) u();
    };
  });
</script>

{#if visible}
  {#if mode === "agentic"}
    <aside class="agent" style={`width:${width}px; position: relative;`}>
      <!-- Drag Handle -->
      <div class="resize-handle" on:mousedown={startResize}></div>

      <header>
        <strong>Agent Panel</strong>
        <select bind:value={activeBackendId} on:change={handleBackendChange} class="backend-select">
          {#each backends as b}
            <option value={b.id}>{b.name} ({b.is_local ? "local" : "remote"})</option>
          {/each}
        </select>
      </header>

      <section class="transcript">
        <div class="assistant">
          <strong>Brick agent is ready.</strong> I can help you with anything on your system — install software, schedule cron jobs, manage services, read/write files, or answer quick questions. Just tell me what you need.
          <br /><br />
          Examples: <em>"install nginx via brew"</em> · <em>"show my cron jobs and add one that runs at midnight"</em> · <em>"what processes are listening on port 8080?"</em>
        </div>

        {#each transcript as item, idx}
          {#if item.type === "user"}
            <div class="user-msg">
              {item.text}
            </div>
          {:else}
            <div class="msg-container">
              {#if item.type === "assistant"}
                <div class="assistant">
                  <p>{item.text}</p>
                  {#if item.contextTrimmed}
                    <span class="context-chip" title="Older context was trimmed to fit inside the LLM context window">Context Trimmed</span>
                  {/if}
                </div>
              {:else if item.type === "proposal"}
                <div class="proposal">
                  <span class="risk">{item.riskTier}</span>
                  {#if editingIndex === idx}
                    <input type="text" bind:value={editedCommand} class="edit-input" />
                  {:else}
                    <code>{item.command}</code>
                  {/if}
                  <footer>
                    {#if editingIndex === idx}
                      <button on:click={() => saveEdit(idx)}><Check size={14} /> Save</button>
                      <button on:click={() => editingIndex = -1}><X size={14} /> Cancel</button>
                    {:else}
                      <button on:click={() => approve(idx, item.command ?? "")} class="btn-approve"><Check size={14} /> Approve</button>
                      <button on:click={() => startEdit(idx, item.command ?? "")}>Edit</button>
                      <button on:click={() => reject(idx)} class="btn-reject"><X size={14} /> Reject</button>
                    {/if}
                  </footer>
                </div>
              {:else if item.type === "blocked"}
                <div class="blocked">
                  <div class="blocked-header">
                    <ShieldAlert size={16} /> <strong>Blocked</strong>
                  </div>
                  <code>{item.command}</code>
                  <p class="reason">{item.reason}</p>
                </div>
              {/if}
            </div>
          {/if}
        {/each}
      </section>

      <footer>
        <textarea
          bind:value={message}
          rows="2"
          placeholder="Ask Brick..."
          disabled={isStreaming}
          on:keydown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
        ></textarea>
        {#if isStreaming}
          <button on:click={cancel} class="btn-cancel"><Square size={14} /> Stop</button>
        {:else}
          <button on:click={send} class="btn-send"><Play size={14} /> Run</button>
        {/if}
      </footer>
    </aside>
  {:else}
    <!-- Helpful side-panel empty state explaining what Agentic mode is -->
    <aside class="agent-empty" style={`width:${width}px;`}>
      <div class="empty-container">
        <ShieldAlert size={36} class="muted-icon" />
        <h3>Agentic Mode Disabled</h3>
        <p>Switch your workspace to <strong>Agentic mode</strong> to use AI-assisted task execution, command rank suggestions, and autonomous terminal steps.</p>
        <button on:click={onEnableAgentic} class="enable-btn">Enable Agentic Mode</button>
      </div>
    </aside>
  {/if}
{/if}

<style>
  .agent-empty {
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    background: #181818;
    border-left: 1px solid #2d2d2d;
    box-sizing: border-box;
    padding: 24px;
    color: #aaaaaa;
  }
  .empty-container {
    text-align: center;
    max-width: 280px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }
  .empty-container h3 {
    font-size: 15px;
    color: #ffffff;
    margin: 0;
  }
  .empty-container p {
    font-size: 12px;
    line-height: 1.5;
    margin: 0;
  }
  .muted-icon {
    color: #444444;
  }
  .enable-btn {
    background-color: #007acc;
    color: white;
    border: none;
    padding: 8px 16px;
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
    font-weight: 500;
    margin-top: 8px;
  }
  .enable-btn:hover {
    background-color: #0062a3;
  }
  .resize-handle {
    position: absolute;
    top: 0;
    left: -4px;
    width: 8px;
    height: 100%;
    cursor: ew-resize;
    z-index: 10;
  }
  .resize-handle:hover {
    background: rgba(165, 109, 25, 0.4);
  }
  .backend-select {
    background: #20241f;
    color: #ecefe7;
    border: 1px solid #3b4038;
    border-radius: 4px;
    padding: 2px 6px;
    font-size: 12px;
    margin-left: auto;
  }
  .user-msg {
    justify-self: end;
    background: #382913;
    color: #ffc15a;
    border: 1px solid #a56d19;
    border-radius: 8px;
    padding: 8px 12px;
    max-width: 80%;
  }
  .msg-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .context-chip {
    font-size: 10px;
    background: #3a3f35;
    color: #a3b899;
    padding: 2px 6px;
    border-radius: 99px;
    align-self: start;
    margin-top: 4px;
    cursor: help;
  }
  .edit-input {
    width: 100%;
    background: #10120f;
    color: #ecefe7;
    border: 1px solid #3b4038;
    border-radius: 4px;
    padding: 6px;
    font-family: monospace;
    margin-top: 6px;
  }
  .btn-approve {
    border-color: #3b7a30;
    color: #c2eec2;
    background: #1a3c14;
  }
  .btn-approve:hover {
    background: #255c1e;
  }
  .btn-reject {
    border-color: #8c2a20;
    color: #ffd0cb;
    background: #4a130f;
  }
  .btn-reject:hover {
    background: #6c1d17;
  }
  .blocked-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }
  .reason {
    font-size: 12px;
    margin: 4px 0 0;
    color: #ff8f7d;
  }
  .btn-send {
    background: #1a3c14;
    border-color: #3b7a30;
  }
  .btn-cancel {
    background: #4a130f;
    border-color: #8c2a20;
  }
</style>
