//! 运行日志（文件持久化）
//!
//! 应用的关键运行事件（启动 / 登录登出 / 下载任务进度与结果 / 错误等）
//! 统一写到这里，供前端「运行日志」页实时阅读，也便于直接打开日志目录人工排查。
//!
//! - 位置：`{config_dir}/LarkReader/logs/`
//! - 文件：`app-YYYY-MM-DD.log`（按天滚动）
//! - 保留：最近 30 天，应用每次启动时清理过期文件
//! - 每条写入后立即 flush，保证正在运行的任务日志前端可实时读到
//! - 日志写失败只降级到 stderr，绝不影响业务逻辑

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// 日志保留天数
const RETENTION_DAYS: i64 = 30;
/// 日志文件名前缀（按天滚动：`app-YYYY-MM-DD.log`）
const FILE_PREFIX: &str = "app";

struct SinkState {
    /// 当前打开的日志文件；跨天时关闭并重建
    file: Option<File>,
    /// 当前打开文件对应的日期（YYYY-MM-DD）
    day: String,
}

struct LogSink {
    state: Mutex<SinkState>,
}

impl LogSink {
    fn new() -> Self {
        Self {
            state: Mutex::new(SinkState {
                file: None,
                day: String::new(),
            }),
        }
    }

    fn write(&self, level: &str, message: &str) {
        let line = format_line(level, message);
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        // 跨天自动滚动到新文件
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if guard.day != today {
            guard.file = None;
            guard.day = today.clone();
        }
        if guard.file.is_none() {
            guard.file = open_today_file(&today);
        }
        if let Some(file) = guard.file.as_mut() {
            if let Err(error) = file.write_all(line.as_bytes()).and_then(|_| file.flush()) {
                eprintln!("[logger] 写入日志失败: {error}");
            }
        }
    }
}

fn sink() -> &'static LogSink {
    static SINK: OnceLock<LogSink> = OnceLock::new();
    SINK.get_or_init(LogSink::new)
}

/// 日志目录：`{config_dir}/LarkReader/logs`
///
/// 与 commands::config_path 同级（都基于 dirs::config_dir()），
/// 保证设置、任务历史、运行日志三处数据放一起，方便用户查找备份。
pub fn log_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("LarkReader")
        .join("logs")
}

/// 初始化日志系统（创建目录 + 清理过期文件），幂等。
/// 应在应用启动时最先调用；即使忘记调用，write 侧也会按需懒创建。
pub fn init() {
    let dir = log_dir();
    if let Err(error) = fs::create_dir_all(&dir) {
        eprintln!("[logger] 无法创建日志目录 {}: {error}", dir.display());
        return;
    }
    cleanup_expired(&dir);
    // 预热单例，确保后续写日志立即可用
    let _ = sink();
}

/// 记录一条 INFO 日志
pub fn info(message: impl AsRef<str>) {
    sink().write("INFO", message.as_ref());
}

/// 记录一条 WARN 日志
pub fn warn(message: impl AsRef<str>) {
    sink().write("WARN", message.as_ref());
}

/// 记录一条 ERROR 日志
pub fn error(message: impl AsRef<str>) {
    sink().write("ERROR", message.as_ref());
}

/// 日志行格式：`YYYY-MM-DD HH:MM:SS.mmm [LEVEL] 消息`
pub(crate) fn format_line(level: &str, message: &str) -> String {
    format!(
        "{} [{}] {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        level,
        message
    )
}

/// 打开（必要时创建）今天的日志文件
fn open_today_file(day: &str) -> Option<File> {
    let path = log_dir().join(format!("{FILE_PREFIX}-{day}.log"));
    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(file) => Some(file),
        Err(error) => {
            eprintln!("[logger] 打开日志文件失败 {}: {error}", path.display());
            None
        }
    }
}

/// 删除保留期之外的旧日志文件
fn cleanup_expired(dir: &Path) {
    let cutoff = chrono::Local::now() - chrono::Duration::days(RETENTION_DAYS);
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("log") {
            continue;
        }
        let expired = entry
            .metadata()
            .ok()
            .and_then(|meta| meta.modified().ok())
            .map(|time| chrono::DateTime::<chrono::Local>::from(time) < cutoff)
            .unwrap_or(false);
        if expired {
            let _ = fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_line;

    #[test]
    fn format_line_contains_timestamp_level_and_message() {
        let line = format_line("INFO", "hello log");
        assert!(line.contains("INFO"));
        assert!(line.contains("hello log"));
        assert!(line.contains('[') && line.contains(']'));
        // 时间戳形如 2026-09-05 22:10:00.123
        let head = &line[..19];
        assert_eq!(head.len(), 19);
        assert_eq!(&line[4..5], "-");
        assert_eq!(&line[13..14], ":");
    }
}
