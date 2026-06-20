# kouik — project context

A note app obsessed with input latency. Keystroke-to-pixel as fast
as the hardware allows. A project for learning close-to-the-metal Rust.

## How I want you to work with me (IMPORTANT)

- I am here to learn.
- Explain the concept and keep me oriented: end each step by showing me the single next small step.

## Principles

- Minimal work per keystroke: input → buffer → draw → present
- Measure before optimizing; record numbers in docs/devlog.md
- Prefer understanding over speed of delivery
- Explain your choices, especially around the render loop

## Stack

- Rust, winit (windowing), wgpu (GPU drawing)
- Local-first, plain text

## Status

- [x] Cargo scaffold + README, pushed to GitHub
- [x] Documentation scaffolding (ARCHITECTURE.md, docs/concepts/)
- [ ] Open a window, clear it to a color
- [ ] Render text
