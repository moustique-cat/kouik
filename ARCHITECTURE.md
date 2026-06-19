# kouik — Architecture

_Living doc. Rewrite this as the design changes; it should describe what the
code does right now, not what you planned._

---

## The one-sentence model

<!-- What does kouik do, end to end, in one sentence? -->

---

## Components

### Windowing — winit

<!-- What does this layer own? What does it give us, and what does it not do? -->

### Text buffer

<!-- How is the note stored in memory? What operations does it need to support? -->

### Renderer — wgpu

<!-- What does this layer own? How does it talk to the GPU? -->

---

## The keystroke-to-pixel path

_This is the most important section. If you can't write these steps, that's
the next thing to learn._

<!-- Number every step from "key physically pressed" to "photon leaves screen."
     Be as concrete as you can: which OS event, which struct, which draw call. -->

1.
2.
3.

---

## The latency budget

_Where do the milliseconds go?_

<!-- Fill in the known fixed costs first, then our per-keystroke work. -->

| Stage | Budget | Notes |
|-------|--------|-------|
| Display refresh (60 Hz) | ~16.7 ms | one frame |
| Compositor hand-off | ? ms | |
| Our per-keystroke work | ? ms | goal: as close to 0 as possible |
