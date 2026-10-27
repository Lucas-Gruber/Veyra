//! In-memory session authority. Lock invalidates every outstanding operation.

use crate::vaultwarden::{ItemMetadata, VaultError, Vaultwarden};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;

pub type ConnectedVault = Arc<AsyncMutex<Vaultwarden>>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub locked: bool,
    pub mode: &'static str,
    pub generation: u64,
    pub last_sync: Option<String>,
}

impl SessionStatus {
    pub fn require_unlocked(&self) -> Result<(), VaultError> {
        if self.locked {
            Err(error("locked", "Unlock the vault first."))
        } else {
            Ok(())
        }
    }
}

pub fn error(code: &'static str, message: &'static str) -> VaultError {
    VaultError {
        code,
        message,
        providers: vec![],
    }
}

pub struct SessionInner {
    pub status: SessionStatus,
    pub vault: Option<ConnectedVault>,
    pub metadata: Vec<ItemMetadata>,
    pub unsupported_items: usize,
    pub cancellation: CancellationToken,
}

impl SessionInner {
    /// Must be called before any login/demo/lock transition. Async completions
    /// need the captured generation and cancellation token to remain valid.
    pub fn invalidate(&mut self) -> Option<ConnectedVault> {
        self.cancellation.cancel();
        self.cancellation = CancellationToken::new();
        self.status.generation += 1;
        self.status.locked = true;
        self.status.mode = "disconnected";
        self.status.last_sync = None;
        self.metadata.clear();
        self.unsupported_items = 0;
        self.vault.take()
    }

    pub fn require_generation(&self, generation: u64) -> Result<(), VaultError> {
        if self.status.generation != generation || self.cancellation.is_cancelled() {
            Err(error(
                "cancelled",
                "This operation was cancelled because the session changed.",
            ))
        } else {
            Ok(())
        }
    }
}

pub struct Session(pub Mutex<SessionInner>);

impl Default for Session {
    fn default() -> Self {
        Self(Mutex::new(SessionInner {
            status: SessionStatus {
                locked: true,
                mode: "disconnected",
                generation: 0,
                last_sync: None,
            },
            vault: None,
            metadata: vec![],
            unsupported_items: 0,
            cancellation: CancellationToken::new(),
        }))
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataSnapshot {
    pub session: SessionStatus,
    pub items: Vec<ItemMetadata>,
    pub unsupported_items: usize,
}

impl SessionInner {
    pub fn snapshot(&self) -> MetadataSnapshot {
        MetadataSnapshot {
            session: self.status.clone(),
            items: if self.status.locked {
                vec![]
            } else {
                self.metadata.clone()
            },
            unsupported_items: if self.status.locked {
                0
            } else {
                self.unsupported_items
            },
        }
    }
}

pub async fn retire(vault: Option<ConnectedVault>) {
    if let Some(vault) = vault {
        vault.lock().await.lock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_is_locked_without_any_account_or_metadata() {
        let session = Session::default();
        let state = session.0.lock().unwrap();
        assert!(state.status.require_unlocked().is_err());
        assert_eq!(state.status.mode, "disconnected");
        assert!(state.snapshot().items.is_empty());
    }

    #[test]
    fn locking_cancels_pending_operations_and_invalidates_their_generation() {
        let session = Session::default();
        let mut state = session.0.lock().unwrap();
        let old_generation = state.status.generation;
        let pending = state.cancellation.clone();
        state.status.locked = false;
        assert!(state.status.require_unlocked().is_ok());
        state.invalidate();
        assert!(pending.is_cancelled());
        assert!(state.require_generation(old_generation).is_err());
        assert!(state.status.require_unlocked().is_err());
    }

    fn metadata() -> ItemMetadata {
        ItemMetadata {
            id: "fixture-only".into(),
            name: "Synthetic item".into(),
            username: "fixture@example.test".into(),
            domain: "example.test".into(),
            url: "https://example.test/".into(),
            kind: "login",
            favorite: false,
            folder: "Test".into(),
            color: "violet",
            initials: "SI".into(),
            updated: "Today".into(),
            tags: vec![],
            has_password: true,
            has_totp: false,
            read_only: true,
        }
    }

    #[test]
    fn locked_snapshot_never_exposes_even_residual_metadata() {
        let session = Session::default();
        let mut state = session.0.lock().unwrap();
        state.metadata.push(metadata());
        state.unsupported_items = 9;
        let snapshot = state.snapshot();
        assert!(snapshot.items.is_empty());
        assert_eq!(snapshot.unsupported_items, 0);
        assert!(snapshot.session.locked);
    }

    #[test]
    fn switching_account_invalidates_previous_generation_and_erases_metadata() {
        let session = Session::default();
        let mut state = session.0.lock().unwrap();
        state.status.locked = false;
        state.status.mode = "connected";
        state.metadata.push(metadata());
        state.unsupported_items = 2;
        let previous = state.status.generation;
        state.invalidate();
        assert!(state.metadata.is_empty());
        assert_eq!(state.unsupported_items, 0);
        assert!(state.require_generation(previous).is_err());
        assert_eq!(state.status.mode, "disconnected");
        assert!(state.status.last_sync.is_none());
    }

    #[tokio::test]
    async fn cancelled_network_work_cannot_complete_after_lock() {
        let session = Session::default();
        let pending = session.0.lock().unwrap().cancellation.clone();
        session.0.lock().unwrap().invalidate();
        let completed = tokio::select! {
            biased;
            _ = pending.cancelled() => false,
            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => true,
        };
        assert!(!completed);
    }
}
