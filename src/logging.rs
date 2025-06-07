use std::{
    fs::{self, OpenOptions},
    io,
    path::PathBuf,
    rc::Rc,
    result::Result,
    sync::mpsc,
    time::{Duration, Instant},
};

use dirs::data_local_dir;
use env_logger::{Builder, Env};

pub const LOG_FILE_NAME: &str = "lazyissues.log";
pub const LOG_DIR_NAME: &str = "lazyissues";

/// call before main logic to start logging
pub fn enable_logging() -> Result<(), std::io::Error> {
    let log_dir = data_local_dir()
        .unwrap_or(PathBuf::new())
        .join(LOG_DIR_NAME);

    fs::create_dir_all(&log_dir)?;

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(LOG_FILE_NAME))?;

    let default_level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };

    Builder::from_env(Env::default().default_filter_or(default_level))
        .target(env_logger::Target::Pipe(Box::new(file)))
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Seconds))
        .init();

    Ok(())
}
