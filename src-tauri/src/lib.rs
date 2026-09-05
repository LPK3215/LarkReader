//! LarkReader — 飞书文档本地阅读器与导出工具
//!
//! 后端入口，注册所有 Tauri 命令。

pub mod commands;
pub mod env;
pub mod error;
pub mod extract;
pub mod lark;
pub mod markdown;
pub mod models;
pub mod wiki;

use std::sync::Mutex;

use commands::AppState;

/// 应用入口
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .compact()
        .init();

    // 加载设置
    let settings = load_settings_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            settings: Mutex::new(settings),
        })
        .invoke_handler(tauri::generate_handler![
            // P0 命令
            commands::check_env,
            commands::setup_lark_cli,
            commands::init_app,
            commands::start_login,
            commands::complete_login,
            commands::login_feishu_blocking,
            commands::preview_doc,
            commands::extract_doc,
            commands::get_settings,
            commands::set_settings,
            // P1 命令
            commands::get_wiki_tree,
            commands::extract_wiki,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LarkReader application");
}

/// 加载设置，如果配置文件不存在则使用默认值
fn load_settings_or_default() -> models::Settings {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("LarkReader");
    std::fs::create_dir_all(&config_dir).ok();

    let config_file = config_dir.join("settings.json");
    match std::fs::read_to_string(&config_file) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => models::Settings::default(),
    }
}
