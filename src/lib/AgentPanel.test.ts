import { render, fireEvent, act, cleanup } from "@testing-library/svelte";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import AgentPanel from "./AgentPanel.svelte";

// Mock Tauri core invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd, args) => {
    if (cmd === "list_backends") {
      return [{ id: "b1", name: "Ollama", kind: "openai", is_default: true, is_local: true }];
    }
    if (cmd === "get_setting") {
      return "360";
    }
    return {};
  }),
}));

// Mock Tauri event listen
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event, callback) => {
    return () => {};
  }),
}));

describe("AgentPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders when visible", async () => {
    const { getByText } = render(AgentPanel, { visible: true, paneId: "pane_123" });
    expect(getByText("Agent")).toBeTruthy();
    expect(getByText("Agentic mode is ready. Ask anything about your repository.")).toBeTruthy();
  });

  it("does not render when invisible", () => {
    const { queryByText } = render(AgentPanel, { visible: false, paneId: "pane_123" });
    expect(queryByText("Agent")).toBeNull();
  });

  it("handles input and textarea keydown Enter to send", async () => {
    const { getByPlaceholderText } = render(AgentPanel, { visible: true, paneId: "pane_123" });
    const textarea = getByPlaceholderText("Ask Brick...") as HTMLTextAreaElement;
    
    await fireEvent.input(textarea, { target: { value: "hello agent" } });
    expect(textarea.value).toBe("hello agent");
    
    await fireEvent.keyDown(textarea, { key: "Enter", shiftKey: false });
    // Textarea gets cleared upon sending
    expect(textarea.value).toBe("");
  });
});
