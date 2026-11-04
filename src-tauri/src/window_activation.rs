//! Native user-initiated activation, separate from focusing a WebView widget.
use tauri::WebviewWindow;

#[cfg(target_os = "linux")]
pub(crate) async fn present(
    window: &WebviewWindow,
    activation_token: Option<String>,
) -> tauri::Result<()> {
    let (send, receive) = tokio::sync::oneshot::channel();
    let popup = window.clone();
    window.run_on_main_thread(move || {
        // A timed-out request must not activate a window later from the queue.
        if send.is_closed() {
            return;
        }
        let result = popup.gtk_window().map(|native| {
            use gtk::prelude::*;
            native.set_accept_focus(true);
            native.set_focus_on_map(true);
            native.realize();
            if let Some(surface) = native.window() {
                if let Ok(x11) = surface.clone().downcast::<gdkx11::X11Window>() {
                    // A global shortcut/CLI launch bypasses GDK's input-event
                    // clock. Tao presents with CURRENT_TIME (zero), which KWin
                    // may reject. Capture one X-server timestamp for this
                    // explicit activation; never cast portal timestamps here.
                    // get_server_time requires property-change delivery.
                    surface
                        .set_events(surface.events() | gtk::gdk::EventMask::PROPERTY_CHANGE_MASK);
                    let timestamp = gdkx11::functions::x11_get_server_time(&x11);
                    x11.set_user_time(timestamp);
                    native.present_with_time(timestamp);
                    return;
                }
                // The display consumes its Wayland activation token exactly
                // once during present/map. Do not retain it on GtkWindow or
                // interpret it as an X11 startup ID/timestamp.
                let display = surface.display();
                if display.type_().name() == "GdkWaylandDisplay" {
                    if let Some(token) = activation_token
                        .as_deref()
                        .filter(|token| valid_token(token))
                    {
                        use gtk::glib::translate::ToGlibPtr;
                        if let Ok(token) = std::ffi::CString::new(token) {
                            let pointer: *mut gtk::gdk::ffi::GdkDisplay = display.to_glib_none().0;
                            // SAFETY: this runs on GTK's main thread; the exact
                            // display GType was checked above, and both the
                            // display and NUL-terminated token outlive the call.
                            // Use the already pinned sys binding rather than
                            // adding a second Wayland client stack for one API.
                            unsafe {
                                gdk_wayland_sys::gdk_wayland_display_set_startup_notification_id(
                                    pointer.cast(),
                                    token.as_ptr(),
                                );
                            }
                        }
                    }
                }
            }
            native.present();
        });
        let _ = send.send(result);
    })?;
    receive
        .await
        .map_err(|_| tauri::Error::FailedToReceiveMessage)?
}

#[cfg(not(target_os = "linux"))]
pub(crate) async fn present(
    window: &WebviewWindow,
    _activation_token: Option<String>,
) -> tauri::Result<()> {
    window.show()?;
    window.unminimize()?;
    window.set_focus()
}

#[cfg(target_os = "linux")]
fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 4096 && !token.chars().any(char::is_control)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn activation_tokens_are_bounded_opaque_values() {
        assert!(super::valid_token("opaque_wayland_token_TIME123"));
        assert!(!super::valid_token(""));
        assert!(!super::valid_token("bad\0token"));
        assert!(!super::valid_token(&"x".repeat(4097)));
    }
}
