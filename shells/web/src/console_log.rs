//! `log` facade → browser DevTools console bridge.
//!
//! Without an installed logger every `log::` call is a silent no-op, so boot
//! diagnostics and fallback warnings never surfaced. This installs a minimal
//! router (called once from `woom24_attach_canvas`); a second install attempt
//! is ignored — the process-global facade keeps its first logger.

use log::{Level, LevelFilter, Log, Metadata, Record};

struct ConsoleLogger;

impl Log for ConsoleLogger
{
    fn enabled(&self, metadata: &Metadata) -> bool { metadata.level() <= Level::Info }

    fn log(&self, record: &Record)
    {
        if !self.enabled(record.metadata()) { return; }
        let line = format!("woom24: {}", record.args());
        match record.level()
        {
            // F9 §4 crash visibility (defect E): on wasm this channel IS the
            // I_Error route (i_system.rs I_Error log::error!s the fatal
            // message before exiting). Page rendering of the error is owned
            // by the woom24_last_error channel + the woom24.js overlay
            // (spec-4 E, the canonical crash-visibility surface); this
            // logger only feeds DevTools (gate 3's data source).
            Level::Error => web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&line)),
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
pub fn install_once()
{
    install_panic_hook();
    let _ = log::set_boxed_logger(Box::new(ConsoleLogger));
    log::set_max_level(LevelFilter::Info);
}

//? DEBUG(probe): the default wasm panic hook only traps ("unreachable executed")
//? with no location, which left the browser boot freeze undiagnosable. This hook
//? surfaces the exact file:line of every panic in DevTools. Candidate to keep
//? permanently as the data source for F8's in-framebuffer error screen.
fn install_panic_hook()
{
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info|
    {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>()
        {
            (*s).to_string()
        }
        else if let Some(s) = info.payload().downcast_ref::<String>()
        {
            s.clone()
        }
        else
        {
            "non-string panic payload".to_string()
        };
        web_sys::console::error_1(&format!("[woom24 panic] {} at {}", payload, location).into());
        default_hook(info);
    }));
}
