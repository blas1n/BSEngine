use std::sync::OnceLock;

static LOGGING_INIT: OnceLock<()> = OnceLock::new();

pub fn init_logging() {
    LOGGING_INIT.get_or_init(|| {
        use tracing_subscriber::{fmt, EnvFilter};
        fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("bsengine=debug,warn")),
            )
            .init();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_can_be_called_multiple_times() {
        // OnceLock ensures second call is a no-op, not a panic
        init_logging();
        init_logging();
    }

    #[test]
    fn logging_macros_work_after_init() {
        init_logging();
        tracing::info!("bsengine-core logging test");
        tracing::debug!("debug message");
        tracing::warn!("warn message");
        // If we reach here without panic, logging is working
    }
}
