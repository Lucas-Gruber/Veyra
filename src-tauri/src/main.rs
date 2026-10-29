#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
fn configure_linux_graphics() {
    // Before GTK or worker threads start, use the same compatibility defaults as
    // Noryn. Respect explicit overrides, including native Wayland installations.
    if std::env::var_os("GDK_BACKEND").is_none()
        && std::env::var_os("DISPLAY").is_some_and(|display| !display.is_empty())
    {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    // WebKitGTK's NVIDIA explicit-sync/DMA-BUF path can terminate the process
    // with Wayland protocol error 71. Keep the WebKit sandbox enabled.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    configure_linux_graphics();

    veyra_lib::run();
}
