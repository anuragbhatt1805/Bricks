<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Check, AlertCircle, RefreshCw } from "lucide-svelte";
  import type { LlmBackendConfig } from "./types";

  let tab: "Backends" | "Shell Integration" | "Data" | "Appearance" = "Backends";
  const tabs = ["Backends", "Shell Integration", "Data", "Appearance"] as const;

  // Backend form state
  let backendName = "Ollama";
  let baseUrl = "http://localhost:11434";
  let model = "llama3.2";
  let isLocal = true;
  let savedId = "default_ollama";

  let saving = false;
  let saveError = "";
  let saveSuccess = false;

  let testing = false;
  let testResult: { ok: boolean; message: string } | null = null;

  async function loadBackend() {
    try {
      const backends = await invoke<LlmBackendConfig[]>("list_backends");
      const def = backends.find((b) => b.is_default) ?? backends[0];
      if (def) {
        savedId = def.id;
        backendName = def.name;
        baseUrl = def.base_url ?? "";
        model = def.model;
        isLocal = def.is_local;
      }
    } catch (e) {
      // No backends yet — defaults are fine
    }
  }

  async function handleSave() {
    saving = true;
    saveError = "";
    saveSuccess = false;
    try {
      const config: LlmBackendConfig = {
        id: savedId,
        name: backendName,
        kind: "openai",
        base_url: baseUrl,
        model,
        api_key_ref: null,
        is_default: true,
        is_local: isLocal,
        created_at: Date.now(),
        context_window_tokens: 12000,
      };
      await invoke("save_backend", { config });
      saveSuccess = true;
      setTimeout(() => { saveSuccess = false; }, 3000);
    } catch (e: any) {
      saveError = e?.toString() ?? "Unknown error saving backend";
    } finally {
      saving = false;
    }
  }

  async function handleTest() {
    testing = true;
    testResult = null;
    try {
      // Save first so the registry is current
      await handleSave();
      const backends = await invoke<LlmBackendConfig[]>("list_backends");
      const def = backends.find((b) => b.is_default);
      if (!def) throw new Error("No default backend found after save");
      const latencyMs = await invoke<number>("test_backend_connection", { backendId: def.id });
      testResult = { ok: true, message: `Connected! Latency: ${latencyMs}ms` };
    } catch (e: any) {
      testResult = { ok: false, message: e?.toString() ?? "Connection failed" };
    } finally {
      testing = false;
    }
  }

  onMount(loadBackend);
</script>

<section class="settings">
  <!-- Tab bar -->
  <nav class="tab-bar">
    {#each tabs as item}
      <button class="tab" class:active={tab === item} on:click={() => (tab = item)}>
        {item}
      </button>
    {/each}
  </nav>

  <!-- Backends tab -->
  {#if tab === "Backends"}
    <div class="pane">
      <h2>LLM Configuration</h2>
      <p class="hint">Connect to a local or remote LLM server to enable Agentic mode.</p>

      <label>
        <span>Name</span>
        <input bind:value={backendName} placeholder="e.g. Ollama" />
      </label>

      <label>
        <span>Base URL</span>
        <input bind:value={baseUrl} placeholder="e.g. http://localhost:11434" />
      </label>

      <label>
        <span>Model ID</span>
        <input bind:value={model} placeholder="e.g. llama3.2 or gemma4:12b" />
      </label>

      <label class="checkbox-row">
        <input type="checkbox" bind:checked={isLocal} />
        <span>Local server (no HTTPS required)</span>
      </label>

      <div class="btn-row">
        <button class="btn-primary" on:click={handleSave} disabled={saving}>
          {saving ? "Saving…" : "Save & Set Default"}
        </button>
        <button class="btn-secondary" on:click={handleTest} disabled={testing || saving}>
          {#if testing}
            <RefreshCw size={13} class="spin" /> Testing…
          {:else}
            Test Connection
          {/if}
        </button>
      </div>

      {#if saveError}
        <div class="msg error"><AlertCircle size={14} /> {saveError}</div>
      {/if}

      {#if saveSuccess && !testResult}
        <div class="msg success"><Check size={14} /> Saved successfully</div>
      {/if}

      {#if testResult}
        <div class="msg" class:success={testResult.ok} class:error={!testResult.ok}>
          {#if testResult.ok}<Check size={14} />{:else}<AlertCircle size={14} />{/if}
          {testResult.message}
        </div>
      {/if}
    </div>

  {:else if tab === "Shell Integration"}
    <div class="pane">
      <h2>Shell Hook</h2>
      <p class="hint">
        The zsh hook allows Brick to capture command history, exit codes, and git context.
      </p>
      <p class="code-block">cp scripts/brick_hook.zsh ~/.config/brick/brick_hook.zsh</p>
      <p class="code-block">echo 'source ~/.config/brick/brick_hook.zsh' >> ~/.zshrc</p>
    </div>

  {:else if tab === "Data"}
    <div class="pane">
      <h2>Local Data</h2>
      <p class="hint">All data is stored locally on your machine. No cloud sync.</p>
      <p class="code-block">~/Library/Application Support/dev.brick.app/brick.db</p>
    </div>

  {:else}
    <div class="pane">
      <h2>Appearance</h2>
      <label class="range-row">
        Font size
        <input type="range" min="10" max="22" value="13" />
      </label>
    </div>
  {/if}
</section>

<style>
  .settings {
    flex: 1;
    display: flex;
    flex-direction: column;
    background-color: #1e1e1e;
    color: #cccccc;
    overflow: hidden;
  }

  /* ── Tab bar ─────────────────────────────────────── */
  .tab-bar {
    display: flex;
    border-bottom: 1px solid #2d2d2d;
    background-color: #252526;
    padding: 0 16px;
    flex-shrink: 0;
  }
  .tab {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: #858585;
    padding: 10px 16px;
    font-size: 13px;
    cursor: pointer;
    transition: color 0.15s, border-color 0.15s;
    white-space: nowrap;
  }
  .tab:hover { color: #cccccc; }
  .tab.active {
    color: #ffffff;
    border-bottom-color: #007acc;
  }

  /* ── Content pane ────────────────────────────────── */
  .pane {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 480px;
  }
  h2 {
    font-size: 16px;
    font-weight: 600;
    color: #ffffff;
    margin: 0;
  }
  .hint {
    font-size: 12px;
    color: #858585;
    margin: 0;
    line-height: 1.5;
  }

  /* ── Form fields ─────────────────────────────────── */
  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  label span {
    font-size: 12px;
    color: #aaaaaa;
  }
  input:not([type="checkbox"]):not([type="range"]) {
    background-color: #3c3c3c;
    border: 1px solid #555555;
    border-radius: 4px;
    color: #ffffff;
    font-size: 13px;
    padding: 7px 10px;
    font-family: inherit;
    transition: border-color 0.15s;
  }
  input:not([type="checkbox"]):not([type="range"]):focus {
    outline: none;
    border-color: #007acc;
  }
  .checkbox-row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .range-row {
    flex-direction: row;
    align-items: center;
    gap: 12px;
    font-size: 13px;
  }

  /* ── Button row ──────────────────────────────────── */
  .btn-row {
    display: flex;
    gap: 10px;
    margin-top: 4px;
  }
  .btn-primary, .btn-secondary {
    border: none;
    border-radius: 4px;
    font-size: 13px;
    font-weight: 500;
    padding: 8px 18px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    transition: background-color 0.15s;
  }
  .btn-primary { background-color: #007acc; color: #fff; }
  .btn-primary:hover:not(:disabled) { background-color: #005fa3; }
  .btn-secondary { background-color: #3e3e3f; color: #cccccc; }
  .btn-secondary:hover:not(:disabled) { background-color: #505055; }
  button:disabled { opacity: 0.55; cursor: not-allowed; }

  /* ── Status messages ─────────────────────────────── */
  .msg {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-radius: 4px;
    font-size: 13px;
  }
  .msg.success { background: rgba(13,188,121,.12); color: #0dbc79; border: 1px solid rgba(13,188,121,.3); }
  .msg.error   { background: rgba(205,49,49,.12);  color: #f17070; border: 1px solid rgba(205,49,49,.3);  }

  /* ── Code blocks (mono display) ──────────────────── */
  .code-block {
    font-family: monospace;
    font-size: 12px;
    background: #2d2d2d;
    border-radius: 4px;
    padding: 8px 12px;
    color: #d4d4d4;
    margin: 0;
  }

  :global(.spin) { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
