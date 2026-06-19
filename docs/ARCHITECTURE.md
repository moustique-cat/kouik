# kouik — Architecture

_Living doc. Rewrite this as the design changes; it should describe what the
code does right now, not what is planned._

---

## The one-sentence model

A keystroke enters the OS, updates an in-memory text buffer, is rendered to a GPU surface via wgpu, and is presented to the display — all within a single frame budget.

---

## Components

### Windowing — winit

Owns everything the OS owns: window creation, title, size, and the event loop (keystrokes, mouse, resize, close). It knows nothing about drawing. Its output is a platform window handle that a graphics API can target.

Pinned version: `winit = "0.30"`. Breaking change from 0.29: `EventLoop::run` was replaced by `EventLoop::run_app` and an `ApplicationHandler` trait — closure-based tutorials for 0.29 are outdated.

### Text buffer

<!-- How is the note stored in memory? What operations does it need to support? -->

### Renderer — wgpu

Owns the pixels. A Rust implementation of the WebGPU API that dispatches to Metal (macOS), Vulkan, or DX12 through a single portable interface. Knows nothing about windows — it receives a `wgpu::Surface` created from winit's window handle, clears or draws into it, and calls `present()` to hand the frame to the compositor.

Pinned version: `wgpu = { version = "24", features = ["wgsl"] }`. The `wgsl` feature enables runtime shader compilation; WGSL is the shader language used for all draw calls.

`pollster = "0.4"` is also required: wgpu initialization is `async`, and pollster lets us block a synchronous thread on those futures without pulling in a full async runtime.

---

## The keystroke-to-pixel path

_This is the most important section. If you can't write these steps, that's
the next thing to learn._

<!-- Number every step from "key physically pressed" to "photon leaves screen."
     Be as concrete as you can: which OS event, which struct, which draw call.
     Fill this in as you build each layer. -->

1. Key physically pressed → OS generates keyboard event
2. winit event loop receives event, delivers it as `WindowEvent::KeyboardInput`
3. Application updates in-memory text buffer
4. Application requests a redraw (`window.request_redraw()`)
5. winit delivers `WindowEvent::RedrawRequested`
6. wgpu acquires the next surface texture
7. A render pass clears the frame (and later: draws glyphs)
8. `present()` hands the frame to the OS compositor
9. Compositor schedules display at next VSync → photon leaves screen

---

## The latency budget

_Where do the milliseconds go?_

<!-- Fill in the known fixed costs first, then our per-keystroke work. -->

| Stage | Budget | Notes |
|-------|--------|-------|
| Display refresh (60 Hz) | ~16.7 ms | one frame |
| Compositor hand-off | ? ms | |
| Our per-keystroke work | ? ms | goal: as close to 0 as possible |
