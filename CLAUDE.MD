# kouik — project context

A note app obsessed with input latency. Keystroke-to-pixel as fast
as the hardware allows. Built to learn close-to-the-metal Rust.

## Principles
- Minimal work per keystroke: input → buffer → draw → present
- Measure before optimizing; record numbers in docs/devlog.md
- Prefer understanding over speed of delivery — explain choices to me,
  especially anything touching the render loop or latency

## Stack
- Rust, winit (windowing), wgpu (GPU drawing)
- Local-first, plain text

## Status
- [ ] Open a window, clear to a color
- [ ] Render text