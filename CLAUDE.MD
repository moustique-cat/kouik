# kouik — project context

A note app obsessed with input latency. Keystroke-to-pixel as fast as the hardware allows.

## How I want you to work

- Commit often. Only one line message.

## Principles

- Minimal work per keystroke: input → buffer → draw → present
- Prefer understanding over speed of delivery

## Stack

- Rust, winit (windowing), wgpu (GPU drawing)
- You can consider other librairies or dependencies if necessary for the job. The constraint is that they must be well known and safe.
- Local-first, plain text

## Design

- Specs live in `design/` (palette.html, type.html), committed to main
- Synced to claude.ai/design (URLs in memory)

## Status

- [x] Cargo scaffold + README, pushed to GitHub
- [x] Documentation scaffolding (ARCHITECTURE.md, docs/concepts/)
- [ ] Open a window, clear it to a color
- [ ] Render text
