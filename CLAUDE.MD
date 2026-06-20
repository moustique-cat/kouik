# kouik — project context

A note app obsessed with input latency. Keystroke-to-pixel as fast as the hardware allows.

## How I want you to work
- Commit often. Only one line message. 

## Principles

- Minimal work per keystroke: input → buffer → draw → present
- Prefer understanding over speed of delivery

## Stack

- Rust, winit (windowing), wgpu (GPU drawing)
- Local-first, plain text

## Design

- Specs live in `design/` (palette.html, type.html), committed to main
- Synced to claude.ai/design project **"kouik"** (projectId: `02884aad-6444-46f1-8d09-163c1a546d5a`)
- To update: edit `design/*.html`, then use DesignSync `finalize_plan` → `write_files` → `register_assets`

## Status

- [x] Cargo scaffold + README, pushed to GitHub
- [x] Documentation scaffolding (ARCHITECTURE.md, docs/concepts/)
- [ ] Open a window, clear it to a color
- [ ] Render text
