<script lang="ts">
  import { X, Check, Square, Play, Trash2, ShieldAlert } from "lucide-svelte";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  export let visible = false;
  export let paneId = "";

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

    async function onMouseUp() {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
      await invoke("set_setting", { key: "agent.panel_width", value: width.toString() });
    }

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  }

  async function send() {
    if (!message.trim() || isStreaming || !paneId) return;
    let userMsg = message;
    message = "";

    transcript = [...transcript, { type: "user", text: userMsg }];
    isStreaming = true;

    // Add empty assistant turn
    transcript = [...transcript, { type: "assistant", text: "", done: false }];

    try {
      await invoke("agent_run_turn", { paneId, userMessage: userMsg });
    } catch (e) {
      console.error(e);
      let lastIdx = transcript.length - 1;
      if (lastIdx >= 0) {
        transcript[lastIdx].text = `Error running turn: ${e}`;
        transcript[lastIdx].done = true;
      }
      isStreaming = false;
    }
  }

  async function cancel() {
    if (!paneId) return;
    await invoke("cancel_agent_turn", { paneId });
    isStreaming = false;
  }

  async function approve(index: number, cmd: string) {
    if (!paneId) return;
    await invoke("agent_approve_command", { paneId, editedCommand: null });
    // convert proposal to assistant narrative
    transcript[index].type = "assistant";
    transcript[index].text = `Executing: ${cmd}`;
    transcript = [...transcript];
    isStreaming = true;
  }

  function startEdit(index: number, cmd: string) {
    editingIndex = index;
    editedCommand = cmd;
  }

  async function saveEdit(index: number) {
    if (!paneId) return;
    transcript[index].command = editedCommand;
    editingIndex = -1;
    await invoke("agent_approve_command", { paneId, editedCommand });
    transcript[index].type = "assistant";
    transcript[index].text = `Executing: ${editedCommand}`;
    transcript = [...transcript];
    isStreaming = true;
  }

  async function reject(index: number) {
    if (!paneId) return;
    await invoke("agent_reject_command", { paneId });
    transcript.splice(index, 1);
    transcript = [...transcript];
    isStreaming = false;
  }

  let unlistens: (() => void)[] = [];

  onMount(() => {
    let unmounted = false;

    async function init() {
      await loadWidth();
      if (unmounted) return;
      await loadBackends();
      if (unmounted) return;

      let u1 = await listen<any>("agent_stream_chunk", (event) => {
        let payload = event.payload;
        if (payload.pane_id !== paneId) return;

        let lastIdx = transcript.length - 1;
        if (lastIdx >= 0 && transcript[lastIdx].type === "assistant") {
          transcript[lastIdx].text += payload.delta;
          transcript[lastIdx].contextTrimmed = payload.context_trimmed;
          if (payload.done) {
            transcript[lastIdx].done = true;
          }
          transcript = [...transcript];
        }
      });

      let u2 = await listen<any>("agent_proposed_command", (event) => {
        let payload = event.payload;
        if (payload.pane_id !== paneId) return;
        transcript = [...transcript, {
          type: "proposal",
          command: payload.command,
          riskTier: payload.risk_tier,
        }];
        isStreaming = false;
      });

      let u3 = await listen<any>("agent_blocked_command", (event) => {
        let payload = event.payload;
        if (payload.pane_id !== paneId) return;
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
  <aside class="agent" style={`width:${width}px; position: relative;`}>
    <!-- Drag Handle -->
    <div class="resize-handle" on:mousedown={startResize}></div>

    <header>
      <strong>Agent</strong>
      <select bind:value={activeBackendId} on:change={handleBackendChange} class="backend-select">
        {#each backends as b}
          <option value={b.id}>{b.name} ({b.is_local ? "local" : "remote"})</option>
        {/each}
      </select>
    </header>

    <section class="transcript">
      <div class="assistant">Agentic mode is ready. Ask anything about your repository.</div>

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
{/if}

<style>
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
