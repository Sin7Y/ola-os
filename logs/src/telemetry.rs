use std::panic;

use tracing::{subscriber::set_global_default, Subscriber};
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

pub fn get_subscriber(
    name: String,
    env_filter: String,
) -> (impl Subscriber + Send + Sync, WorkerGuard) {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let mut base_path = std::env::current_dir().expect("Failed to determine the current directory");
    base_path.push(".logs");
    let file_appender = tracing_appender::rolling::hourly(base_path, "olaos.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let file_layer = BunyanFormattingLayer::new(name, non_blocking);
    let res = Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(file_layer);
    (res, guard)
}

/// Register a subscriber as global default to process span data.
///
/// It should only be called once!
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    // Redirect all `log`'s events to our subscriber
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber");
}

pub fn set_panic_hook() {
    let placeholder = "Unknown panic info".to_string();
    panic::set_hook(Box::new(move |panic_info| {
        let payload = panic_info
            .payload()
            .downcast_ref::<String>()
            .unwrap_or(&placeholder);
        let location = panic_info
            .location()
            .unwrap_or_else(|| panic::Location::caller());
        let panic_message = format!(
            "Panic occurred in file '{}' at line {}: {}",
            location.file(),
            location.line(),
            payload
        );

        super::error!("{}", panic_message);
    }));
}
