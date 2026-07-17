pub mod account_store;
pub mod activity;
pub mod auth_vault;
pub mod backup;
pub mod ccswitch_import;
pub mod cli_status;
pub mod config_writer;
pub mod error;
pub mod health;
pub mod mask;
pub mod normalize;
pub mod paths;
pub mod provider_store;
pub mod settings_store;
pub mod skill_store;
pub mod terminal;
pub mod types;

pub use error::AppError;
pub use mask::mask_secret;
pub use normalize::{
    gs_model_key, is_safe_model_token, normalize_base_url, sanitize_model_name,
    validate_model_token,
};
pub use paths::Paths;
pub use types::*;

use std::sync::{Mutex, MutexGuard};

/// Process-wide serialization for read-modify-write operations on the JSON
/// stores (providers / accounts / settings / activity).
///
/// Tauri commands run on a thread pool and the tray can trigger switches
/// concurrently with the UI, so two "read → mutate → write" sequences could
/// otherwise interleave and silently drop one update. Callers acquire this at
/// the top of a mutation and hold it across the whole read+write.
///
/// Only top-level mutation entry points acquire it; internal helpers must not,
/// since `std::sync::Mutex` is non-reentrant (that would deadlock).
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the global store write lock, recovering from a poisoned mutex so a
/// panic in one operation cannot wedge all future writes.
pub fn lock_store() -> MutexGuard<'static, ()> {
    STORE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
