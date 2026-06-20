use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoop;
use winit::window::Window;
use winit::window::WindowId;

struct App {
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // called once when app is ready
        println!("App resumed");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // called for every window event
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // called when event queue is empty
    }
}

fn main() {
    let mut app = App { window: None };
    println!("App created");
    let event_loop = EventLoop::new().unwrap();
    println!("Event loop created");
    event_loop.run_app(&mut app);
}
