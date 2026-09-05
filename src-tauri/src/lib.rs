//! LarkReader — 飞书文档本地阅读器与导出工具
//!
//! 后端入口，注册所有 Tauri 命令。

pub mod commands;
pub mod env;
pub mod error;
pub mod extract;
pub mod lark;
pub mod logger;
pub mod markdown;
pub mod models;
pub mod reader;
pub mod wiki;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use commands::AppState;

/// 应用入口
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化运行日志（文件持久化，供前端「运行日志」页实时阅读）
    logger::init();
    logger::info(format!(
        "{} {} 启动",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    ));

    // 初始化控制台日志（开发期调试用，正式日志以 logger.rs 文件为准）
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .compact()
        .try_init()
        .ok();

    // 加载设置
    let (settings, settings_warning) = load_settings_or_default();
    if let Some(warning) = &settings_warning {
        logger::warn(warning);
    }
    let completed_tasks = load_task_history();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(settings),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            completed_tasks: Arc::new(Mutex::new(completed_tasks)),
            settings_warning: Mutex::new(settings_warning),
            app_init: Arc::new(Mutex::new(models::AppInitStatus::default())),
            last_tree: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            // P0 命令
            commands::check_env,
            commands::setup_lark_cli,
            commands::start_app_init,
            commands::get_app_init_status,
            commands::start_login,
            commands::complete_login,
            commands::logout,
            commands::preview_doc,
            commands::extract_doc,
            commands::get_settings,
            commands::set_settings,
            commands::get_settings_status,
            commands::preflight_output_dir,
            commands::open_output_dir,
            // P1 命令
            commands::get_wiki_tree,
            commands::count_exportable,
            commands::extract_wiki,
            commands::get_progress,
            commands::cancel_task,
            commands::start_extract_wiki,
            commands::get_task_result,
            commands::dismiss_task_result,
            commands::list_task_history,
            // 运行日志
            commands::list_log_files,
            commands::read_log_file,
            commands::open_log_dir,
            // 本地阅读（Reader）
            commands::list_reader_dir,
            commands::read_reader_md,
            commands::read_reader_binary,
            commands::find_first_reader_doc,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LarkReader application");
}

/// 加载设置，如果配置文件不存在则使用默认值
fn load_settings_or_default() -> (models::Settings, Option<String>) {
    let config_file = commands::config_path();
    match std::fs::read_to_string(&config_file) {
        Ok(content) => match serde_json::from_str::<models::Settings>(&content) {
            Ok(settings) => (settings, None),
            Err(error) => {
                let backup = config_file.with_extension(format!(
                    "corrupt-{}.json",
                    chrono::Utc::now().format("%Y%m%d%H%M%S")
                ));
                let backup_message = match std::fs::rename(&config_file, &backup) {
                    Ok(()) => format!("，原文件已备份到 {}", backup.display()),
                    Err(backup_error) => format!("，且备份失败: {backup_error}"),
                };
                (
                    models::Settings::default(),
                    Some(format!(
                        "配置文件损坏，已恢复默认设置: {error}{backup_message}"
                    )),
                )
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (models::Settings::default(), None)
        }
        Err(error) => (
            models::Settings::default(),
            Some(format!("读取配置失败，已使用默认设置: {error}")),
        ),
    }
}

fn load_task_history() -> HashMap<String, models::WikiTaskResult> {
    let mut tasks = std::fs::read_to_string(commands::history_path())
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default();
    commands::clean_completed_tasks(&mut tasks);
    tasks
}
