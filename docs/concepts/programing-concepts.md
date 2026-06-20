# What an event loop actually is

Your program doesn't run straight through and exit. Instead, winit takes over the thread and spins in a loop forever, waiting for things to happen (key pressed, mouse moved, close button clicked). Each time something happens, it calls your code with a description of the event. When your code returns, winit goes back to waiting.

That loop is EventLoop. You give it control at the end of main with run_app, and it never returns until the app exits.

# Why a struct, and why a trait

winit needs to call your code when events arrive. In winit 0.29, you handed it a closure. In 0.30, you hand it a struct, and winit calls methods on that struct.

A trait in Rust is a list of methods that a struct promises to have. ApplicationHandler is winit's trait — it says "any struct that wants to receive events must have these three methods: resumed, window_event, about_to_wait."

You define a struct, write those three methods on it, and then winit can call them. That's all "implement a trait" means: you're writing the methods that winit's trait requires.

# Why Option<Window>

Your struct exists before the window does. You create the struct in main, then hand it to run_app. Only after that does winit call resumed, which is your signal to create the window.

So at the moment your struct is born, it has no window yet. Option<Window> means "either a window, or nothing." You start with None, and inside resumed you replace it with Some(the_window).

The skeleton, explained line by line
Here's the shape of the code, with every line explained — but the actual code is yours to write:

```rust
struct App {
    window: Option<Window>,   // None until resumed() runs
}
```

Then you write impl ApplicationHandler for App { ... } which is Rust syntax for "here are the three methods winit requires." Inside that block:

resumed method receives an event_loop argument. You call event_loop.create_window(...) to get a Window, then store it in self.window.

window_event method receives an event argument. You write a match event { ... } that looks for WindowEvent::CloseRequested and calls event_loop.exit() when it sees it. All other events you ignore with _ => {}.

about_to_wait method receives event_loop but you don't use it yet. Body is empty: {}.

In main:

EventLoop::new() — creates the loop (returns a Result, so unwrap it).
Create your struct with window: None.
event_loop.run_app(&mut app) — hands control to winit.
