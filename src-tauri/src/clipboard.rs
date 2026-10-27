//! Backend-owned, best-effort clipboard expiry. External history is outside our control.

use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const CLEAR_AFTER: Duration = Duration::from_secs(30);

type SafeResult = Result<(), String>;

trait ClipboardBackend {
    fn write(&mut self, value: &str) -> SafeResult;
    fn read(&mut self) -> Result<Zeroizing<String>, String>;
    fn clear(&mut self) -> SafeResult;
}

impl ClipboardBackend for arboard::Clipboard {
    fn write(&mut self, value: &str) -> SafeResult {
        self.set_text(value)
            .map_err(|_| "Clipboard is unavailable in this desktop session.".into())
    }

    fn read(&mut self) -> Result<Zeroizing<String>, String> {
        self.get_text()
            .map(Zeroizing::new)
            .map_err(|_| "Clipboard cannot be read in this desktop session.".into())
    }

    fn clear(&mut self) -> SafeResult {
        arboard::Clipboard::clear(self).map_err(|_| "Clipboard could not be cleared.".into())
    }
}

struct Pending {
    value: Zeroizing<String>,
    expires_at: Instant,
}

struct ClipboardManager<C> {
    backend: C,
    pending: Option<Pending>,
}

impl<C: ClipboardBackend> ClipboardManager<C> {
    fn new(backend: C) -> Self {
        Self {
            backend,
            pending: None,
        }
    }

    fn write(&mut self, value: Zeroizing<String>, now: Instant) -> SafeResult {
        self.backend.write(&value)?;
        self.pending = Some(Pending {
            value,
            expires_at: now + CLEAR_AFTER,
        });
        Ok(())
    }

    fn expire(&mut self, now: Instant) {
        if self.pending.as_ref().is_some_and(|p| now >= p.expires_at) {
            self.clear_if_unchanged();
        }
    }

    fn clear_if_unchanged(&mut self) {
        if let Some(pending) = self.pending.take() {
            if let Ok(current) = self.backend.read() {
                if *current == *pending.value {
                    // Best effort: read and clear are not an atomic operation on Linux.
                    let _ = self.backend.clear();
                }
            }
        }
    }
}

enum Request {
    Write {
        value: Zeroizing<String>,
        reply: Sender<SafeResult>,
    },
    Clear,
}

pub struct ClipboardService {
    sender: Sender<Request>,
}

impl Default for ClipboardService {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || clipboard_worker(receiver));
        Self { sender }
    }
}

impl ClipboardService {
    pub fn write(&self, value: Zeroizing<String>) -> SafeResult {
        let (reply, response) = mpsc::channel();
        self.sender
            .send(Request::Write { value, reply })
            .map_err(|_| "Clipboard service is unavailable.".to_owned())?;
        response
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "Clipboard operation timed out.".to_owned())?
    }

    pub fn clear(&self) {
        let _ = self.sender.send(Request::Clear);
    }
}

fn clipboard_worker(receiver: Receiver<Request>) {
    // Create lazily: startup and tests must also work without a display server.
    // Keep the owner alive for the process lifetime so Linux can serve paste requests.
    let mut clipboard: Option<ClipboardManager<arboard::Clipboard>> = None;
    loop {
        match receiver.recv_timeout(Duration::from_millis(250)) {
            Ok(Request::Write { value, reply }) => {
                if clipboard.is_none() {
                    match arboard::Clipboard::new() {
                        Ok(backend) => clipboard = Some(ClipboardManager::new(backend)),
                        Err(_) => {
                            let _ = reply.send(Err(
                                "Clipboard is unavailable. Wayland may require supported data-control or XWayland."
                                    .into(),
                            ));
                            continue;
                        }
                    }
                }
                if let Some(manager) = clipboard.as_mut() {
                    let _ = reply.send(manager.write(value, Instant::now()));
                }
            }
            Ok(Request::Clear) => {
                if let Some(manager) = clipboard.as_mut() {
                    manager.clear_if_unchanged();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(manager) = clipboard.as_mut() {
            manager.expire(Instant::now());
        }
    }
    if let Some(manager) = clipboard.as_mut() {
        manager.clear_if_unchanged();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryClipboard(String);

    impl ClipboardBackend for MemoryClipboard {
        fn write(&mut self, value: &str) -> SafeResult {
            self.0 = value.to_owned();
            Ok(())
        }
        fn read(&mut self) -> Result<Zeroizing<String>, String> {
            Ok(Zeroizing::new(self.0.clone()))
        }
        fn clear(&mut self) -> SafeResult {
            self.0.clear();
            Ok(())
        }
    }

    #[test]
    fn clears_only_after_the_expiry_interval() {
        let now = Instant::now();
        let mut manager = ClipboardManager::new(MemoryClipboard::default());
        manager.write(Zeroizing::new("demo".into()), now).unwrap();
        manager.expire(now + Duration::from_secs(29));
        assert_eq!(manager.backend.0, "demo");
        manager.expire(now + CLEAR_AFTER);
        assert_eq!(manager.backend.0, "");
    }

    #[test]
    fn does_not_clear_a_clipboard_replaced_by_another_application() {
        let now = Instant::now();
        let mut manager = ClipboardManager::new(MemoryClipboard::default());
        manager.write(Zeroizing::new("demo".into()), now).unwrap();
        manager.backend.0 = "a newly copied document".into();
        manager.expire(now + CLEAR_AFTER);
        assert_eq!(manager.backend.0, "a newly copied document");
    }

    #[test]
    fn a_new_copy_gets_its_own_full_interval() {
        let now = Instant::now();
        let mut manager = ClipboardManager::new(MemoryClipboard::default());
        manager.write(Zeroizing::new("first".into()), now).unwrap();
        manager
            .write(
                Zeroizing::new("second".into()),
                now + Duration::from_secs(20),
            )
            .unwrap();
        manager.expire(now + CLEAR_AFTER);
        assert_eq!(manager.backend.0, "second");
        manager.expire(now + Duration::from_secs(50));
        assert_eq!(manager.backend.0, "");
    }
}
