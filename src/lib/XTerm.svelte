<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";

  export let onInput: (data: string) => void = () => {};
  export let onResize: (cols: number, rows: number) => void = () => {};

  let container: HTMLDivElement;
  let term: Terminal;
  let fitAddon: FitAddon;
  let resizeObserver: ResizeObserver;
  let rafId: number;

  export function write(data: string | Uint8Array) {
    if (term) term.write(data);
  }

  export function clear() {
    if (term) term.clear();
  }

  export function focus() {
    if (term) term.focus();
  }

  function doFit() {
    if (!container || !term || !fitAddon) return;
    // FitAddon needs the container to have non-zero dimensions
    const { clientWidth, clientHeight } = container;
    if (clientWidth === 0 || clientHeight === 0) return;
    fitAddon.fit();
  }

  onMount(() => {
    term = new Terminal({
      cursorBlink: true,
      scrollback: 5000,
      theme: {
        background:  "#1e1e1e",
        foreground:  "#d4d4d4",
        cursor:      "#c7c7c7",
        black:       "#000000",
        red:         "#cd3131",
        green:       "#0dbc79",
        yellow:      "#e5e510",
        blue:        "#2472c8",
        magenta:     "#bc3fbc",
        cyan:        "#11a8cd",
        white:       "#e5e5e5",
        brightBlack: "#666666",
      },
      fontFamily: "Menlo, Monaco, 'Courier New', monospace",
      fontSize: 13,
      lineHeight: 1.2,
    });

    fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(container);

    // Defer initial fit until after the browser has finished the flex layout pass
    rafId = requestAnimationFrame(() => {
      doFit();
      term.focus();
      // Notify parent with accurate dimensions after fit
      onResize(term.cols, term.rows);
    });

    term.onData((data) => {
      onInput(data);
    });

    // Fires whenever cols/rows actually change (after every fitAddon.fit())
    term.onResize(({ cols, rows }) => {
      onResize(cols, rows);
    });

    // Re-fit whenever the container element changes size (window resize, panel toggle, etc.)
    resizeObserver = new ResizeObserver(() => {
      doFit();
    });
    resizeObserver.observe(container);
  });

  onDestroy(() => {
    cancelAnimationFrame(rafId);
    if (resizeObserver) resizeObserver.disconnect();
    if (term) term.dispose();
  });
</script>

<!-- No padding here — padding inside xterm shrinks measured width and breaks fit() -->
<div class="xterm-container" bind:this={container}></div>

<style>
  .xterm-container {
    width: 100%;
    height: 100%;
    overflow: hidden;
    background-color: #1e1e1e;
    box-sizing: border-box;
  }

  /* xterm internal elements must fill the container */
  :global(.xterm) {
    width: 100%;
    height: 100%;
    padding: 6px 8px; /* visual breathing room lives here, NOT on the wrapper */
    box-sizing: border-box;
  }
  :global(.xterm-viewport) {
    background-color: #1e1e1e !important;
    overflow-y: auto !important;
  }
  :global(.xterm-screen) {
    width: 100% !important;
  }
</style>

