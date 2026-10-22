use anyhow::Result;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use tao::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

/// Run the native desktop application window.
pub fn run(preferred_port: Option<u16>) -> Result<()> {
    let port = preferred_port.unwrap_or(7575);
    let addr = format!("127.0.0.1:{port}");

    // Check if a SessionWiki server is already running on this port
    let already_running = TcpStream::connect(&addr).is_ok();
    if !already_running {
        thread::spawn(move || {
            let _ = crate::web::serve(port, true, false);
        });

        // Wait up to 3 seconds for the server to accept connections
        for _ in 0..60 {
            thread::sleep(Duration::from_millis(50));
            if TcpStream::connect(&addr).is_ok() {
                break;
            }
        }
    }

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("SessionWiki")
        .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 920.0))
        .build(&event_loop)?;

    let url = format!("http://{addr}");
    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .build(&window)?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => (),
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => (),
        }
    });
}
