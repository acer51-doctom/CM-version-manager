use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Info,
    Action,
    Warn,
    Error,
}

struct Logger {
    file: File,
}

/// Initializes the global log file
pub fn init(log_file_path: &str) {
    if let Ok(file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
    {
        let mut guard = LOGGER.lock().unwrap();
        *guard = Some(Logger { file });
    }
    info("--- Session Started ---");
}

/// Logs a formatted message with level and timestamp
pub fn log(level: LogLevel, message: impl AsRef<str>) {
    let msg = message.as_ref();
    let timestamp = get_timestamp();
    let level_str = match level {
        LogLevel::Info => "INFO ",
        LogLevel::Action => "ACTION",
        LogLevel::Warn => "WARN ",
        LogLevel::Error => "ERROR",
    };

    let formatted = format!("[{timestamp}] [{level_str}] {msg}\n");

    // Print to stdout / terminal
    print!("{formatted}");

    // Append to cm_manager.log
    if let Ok(mut guard) = LOGGER.lock() {
        if let Some(logger) = guard.as_mut() {
            let _ = logger.file.write_all(formatted.as_bytes());
            let _ = logger.file.flush();
        }
    }
}

// Convenience helpers
pub fn info(msg: impl AsRef<str>) {
    log(LogLevel::Info, msg);
}

pub fn action(msg: impl AsRef<str>) {
    log(LogLevel::Action, msg);
}

pub fn warn(msg: impl AsRef<str>) {
    log(LogLevel::Warn, msg);
}

pub fn error(msg: impl AsRef<str>) {
    log(LogLevel::Error, msg);
}

fn get_timestamp() -> String {
    let start = SystemTime::now();
    let since_epoch = start.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since_epoch.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let seconds = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, seconds)
}