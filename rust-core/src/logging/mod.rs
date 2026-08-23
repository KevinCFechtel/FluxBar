//! Logging setup for the Rust core.
//!
//! Business modules use the `log` crate facade and remain platform agnostic.
//! The macOS backend routes records into Unified Logging under the
//! `dev.kevincfechtel.FluxBar` subsystem. Other platforms use a no-op logger
//! until a platform-specific backend is added.
//!
//! Logging is initialized exactly once from the FFI boundary. Initialization
//! failures are swallowed so a logging problem cannot break core requests.

use std::sync::Once;

static INIT: Once = Once::new();

/// Initializes the process-wide logger exactly once.
///
/// This is called from `FluxCoreRequest` before request processing. It is safe
/// to call from multiple threads and from the panic-recovery path.
pub fn init() {
    INIT.call_once(|| {
        let _ = init_logger();
    });
}

#[cfg(target_os = "macos")]
fn init_logger() -> Result<(), log::SetLoggerError> {
    use log::LevelFilter;
    use oslog::OsLogger;

    OsLogger::new("dev.kevincfechtel.FluxBar")
        .level_filter(LevelFilter::Info)
        .init()
}

#[cfg(not(target_os = "macos"))]
fn init_logger() -> Result<(), log::SetLoggerError> {
    // No-op on unsupported platforms. This keeps the core portable and avoids
    // pulling in macOS-only dependencies for future targets.
    log::set_logger(&NoOpLogger).map(|()| log::set_max_level(log::LevelFilter::Off))
}

#[cfg(not(target_os = "macos"))]
struct NoOpLogger;

#[cfg(not(target_os = "macos"))]
impl log::Log for NoOpLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        false
    }
    fn log(&self, _record: &log::Record) {}
    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_idempotent_and_does_not_panic() {
        init();
        init();
        init();
    }
}
