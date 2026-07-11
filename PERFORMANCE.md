# Performance

## Current Build

- Frontend production bundle: `dist/assets/index-CrEjabnc.js`, 37.09 kB
  uncompressed, 12.17 kB gzip.
- BlockList virtualization test: 10,000 mock blocks render fewer than 50 block
  nodes.
- Rust unit tests: 8 passing.

## Required Manual Profiling Before v1

- macOS Instruments idle memory at launch.
- macOS Instruments memory with 5 tabs open.
- macOS Instruments memory after 200 commands.
- macOS Instruments memory after a long agent session.
- IPC throttle benchmark for `yes` over 10 seconds, target at most 650 events.
- Background pane IPC cadence, target 4 Hz.
- 50,000-block scroll FPS, target above 55 fps.
- `suggest_command` P99 against 100,000 rows, target below 10 ms.
- `lsof -i` with no backend configured, expected zero outbound connections.

## Current Status

The app has the structure required for these measurements, but the manual
Instruments numbers have not been captured in this environment.
