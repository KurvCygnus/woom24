//! `log` facade → browser DevTools console bridge.
//!
//! Without an installed logger every `log::` call is a silent no-op, so boot
//! diagnostics and fallback warnings never surfaced. This installs a minimal
//! router (called once from `woom24_attach_canvas`); a second install attempt
//! is ignored — the process-global facade keeps its first logger.

use log::{Level, LevelFilter, Log, Metadata, Record};

struct ConsoleLogger;

impl Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!("woom24: {}", record.args());
        match record.level() {
            Level::Error => web_sys::console::error_1(&line.into()),
            Level::Warn => web_sys::console::warn_1(&line.into()),
            Level::Info => web_sys::console::info_1(&line.into()),
            Level::Debug | Level::Trace => web_sys::console::log_1(&line.into()),
        }
    }

    fn flush(&self) {}
}

/// Installs the console logger and raises the facade to Info. Safe to call
/// repeatedly: `set_boxed_logger` fails after the first install and the error
/// is deliberately ignored.
pub fn install_once() {
    let _ = log::set_boxed_logger(Box::new(ConsoleLogger));
    log::set_max_level(LevelFilter::Info);
}
