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

    // Check if an AgentWiki server is already running on this port
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
    let icon = {
        let rgba = include_bytes!("../assets/icon_64.rgba");
        tao::window::Icon::from_rgba(rgba.to_vec(), 64, 64).ok()
    };

    let mut builder = WindowBuilder::new()
        .with_title(format!("AgentWiki v{}", env!("CARGO_PKG_VERSION")))
        .with_inner_size(tao::dpi::LogicalSize::new(1440.0, 920.0));
    if let Some(ic) = icon {
        builder = builder.with_window_icon(Some(ic));
    }
    let window = builder.build(&event_loop)?;

    let url = format!("http://{addr}");
    // External links (window.open from the UI, e.g. "Search in Google") are
    // handed to the system browser from THIS process. Doing it via the HTTP
    // server is unreliable: the app may be attached to a server some other
    // process started (see `already_running`), which may not be allowed to
    // launch a browser at all.
    let _webview = WebViewBuilder::new()
        .with_url(&url)
        .with_new_window_req_handler(|target, _features| {
            if target.starts_with("https://") || target.starts_with("http://") {
                crate::web::open_browser(&target);
            }
            wry::NewWindowResponse::Deny
        })
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
