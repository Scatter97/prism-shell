use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn state_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("Prism").join("logs"))
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
            })?;
        Some(base.join("prism-shell").join("logs"))
    }
}

/// Install stderr and best-effort file logging. The returned guard must live
/// for the process lifetime so buffered log lines are flushed.
pub fn init(verbose: bool) -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(if verbose {
            "debug"
        } else {
            "info,prism_app=info,prism_ui=info,prism_platform=info"
        })
    });

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    let file = state_dir().and_then(|directory| {
        std::fs::create_dir_all(&directory).ok()?;
        let appender = tracing_appender::rolling::never(&directory, "prism-preview.log");
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let layer = tracing_subscriber::fmt::layer()
            .with_writer(writer)
            .with_ansi(false)
            .with_target(false);
        Some((layer, guard))
    });

    if let Some((file_layer, guard)) = file {
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .with(file_layer)
            .init();
        Some(guard)
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(stderr_layer)
            .init();
        None
    }
}
