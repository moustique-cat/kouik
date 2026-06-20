# kouik — project context

A note app obsessed with input latency. Keystroke-to-pixel as fast
as the hardware allows.

## How I want you to work
- Commit often. Only one line message. 

## Principles

- Minimal work per keystroke: input → buffer → draw → present
- Prefer understanding over speed of delivery

## Stack

- Rust, winit (windowing), wgpu (GPU drawing)
- Local-first, plain text

## Status

- [x] Cargo scaffold + README, pushed to GitHub
- [x] Documentation scaffolding (ARCHITECTURE.md, docs/concepts/)
- [ ] Open a window, clear it to a color
- [ ] Render text
