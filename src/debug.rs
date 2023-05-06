use std::{
    fs::File,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use color_eyre::section::PanicMessage;
use eyre::{Result, WrapErr};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, prelude::*};

struct DispatchPanicMessage {
    log_path: Arc<Mutex<Option<PathBuf>>>,
}

impl PanicMessage for DispatchPanicMessage {
    fn display(
        &self,
        pi: &std::panic::PanicInfo<'_>,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        use owo_colors::OwoColorize;

        writeln!(
            f,
            "{}",
            "Oh no! dispatch-proxy encountered a critical error and crashed.".red()
        )?;

        let payload = pi
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| pi.payload().downcast_ref::<&str>().cloned())
            .unwrap_or("<non string panic payload>");

        write!(f, "Message:  ")?;
        writeln!(f, "{}", payload.cyan())?;

        // If known, print panic location.
        write!(f, "Location: ")?;
        if let Some(loc) = pi.location() {
            write!(f, "{}", loc.file().purple())?;
            write!(f, ":")?;
            write!(f, "{}", loc.line().purple())?;
        } else {
            write!(f, "<unknown>")?;
        }
        writeln!(f)?;

        if let Some(ref log_path) = *self.log_path.lock().unwrap() {
            writeln!(
                f,
                "A complete history of the events that preceded this error is available at {}",
                log_path.to_string_lossy()
            )?;
        }

        Ok(())
    }
}

fn get_file_writer() -> Result<(PathBuf, NonBlocking, WorkerGuard)> {
    let project_dirs = directories::ProjectDirs::from("", "", "dispatch-proxy")
        .ok_or_else(|| eyre::eyre!("Couldn't find the user's home directory"))?;
    let data_dir = project_dirs.data_local_dir();
    std::fs::create_dir_all(data_dir).wrap_err("Failed to create data directory")?;
    let log_path = data_dir.join("logs.txt");
    let (file_appender, guard) =
        tracing_appender::non_blocking(File::create(&log_path).wrap_err_with(|| {
            format!(
                "Failed to create a log file at {}",
                log_path.to_string_lossy()
            )
        })?);
    Ok((log_path, file_appender, guard))
}

fn init_tracing_subscriber_with_appender(appender: NonBlocking) {
    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_ansi(false)
        .with_writer(appender);

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

fn init_tracing_subscriber_with_stdout() {
    let fmt_layer = fmt::layer().with_target(false);

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

#[derive(Clone, Copy, Debug)]
pub enum LogStrategy {
    File,
    Stdout,
}

pub fn install(log_strategy: LogStrategy) -> Result<Option<WorkerGuard>> {
    use sysinfo::{System, SystemExt};

    let sys = System::new_all();

    std::env::set_var("RUST_LIB_BACKTRACE", "full");

    let shared_log_path = Arc::new(Mutex::new(None));

    color_eyre::config::HookBuilder::default()
        .issue_url(concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"))
        .add_issue_metadata("Version", env!("CARGO_PKG_VERSION"))
        .add_issue_metadata("OS", sys.long_os_version().unwrap_or("Unknown".into()))
        .add_issue_metadata("Command", std::env::args().collect::<Vec<_>>().join(" "))
        .panic_message(DispatchPanicMessage {
            log_path: Arc::clone(&shared_log_path),
        })
        .display_env_section(false)
        .theme(color_eyre::config::Theme::new())
        .install()?;

    match log_strategy {
        LogStrategy::File => match get_file_writer() {
            Ok((log_path, file_appender, guard)) => {
                shared_log_path.lock().unwrap().replace(log_path);

                init_tracing_subscriber_with_appender(file_appender);

                Ok(Some(guard))
            }
            Err(err) => {
                init_tracing_subscriber_with_stdout();

                tracing::error!("{:?}", err);
                tracing::info!(
                    "Failed to access the log file, all logs will be reported here instead."
                );

                Ok(None)
            }
        },
        LogStrategy::Stdout => {
            init_tracing_subscriber_with_stdout();
            Ok(None)
        }
    }
}
