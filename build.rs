#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    let _ = res.compile();
}

#[cfg(not(windows))]
fn main() {}
