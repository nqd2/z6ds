//! Background task spawning — safe from Tauri async commands and GTK/WebKit threads.

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

static BACKGROUND_RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn background_runtime() -> &'static Runtime {
    BACKGROUND_RUNTIME.get_or_init(|| {
        Runtime::new().expect("z6ds-sim background tokio runtime")
    })
}

/// Spawn on a dedicated runtime (never on `Handle::try_current()`).
///
/// Tauri's main / WebKit URI-scheme threads may expose a Tokio `Handle` without a
/// reactor on that thread; `handle.spawn` then panics with "no reactor running".
pub fn spawn_background<F>(future: F) -> JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    background_runtime().spawn(future)
}
