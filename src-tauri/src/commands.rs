//! Tauri 命令注册层
//!
//! 所有暴露给前端的接口都定义在这里。
//! 前端通过 `invoke("command_name", { args })` 调用。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use tauri::State;

use crate::env;
use crate::error::AppError;
use crate::extract;
use crate::models::{
    DeviceInfo, EnvStatus, ExtractResult, LoginResult, PreviewResult, Progress, Settings,
    TaskStatus, WikiExtractResult, WikiNode,
};
use crate::wiki;

/// 应用状态：设置（持久化到本地文件）
pub struct AppState {
    pub settings: Mutex<Settings>,
    pub tasks: Arc<Mutex<HashMap<String, TaskControl>>>,
}

pub struct TaskControl {
    pub progress: Arc<Mutex<Progress>>,
    pub cancelled: Arc<AtomicBool>,
}

/// 配置文件路径
fn config_path() -> std::path::PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("LarkReader");
    std::fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

/// 保存设置到本地文件
fn save_settings(settings: &Settings) -> Result<(), AppError> {
    settings.validate().map_err(AppError::InvalidSetting)?;
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| AppError::Other(format!("设置序列化失败: {}", e)))?;
    let path = config_path();
    let temp_path = path.with_extension("json.tmp");
    let backup_path = path.with_extension("json.bak");
    std::fs::write(&temp_path, content)?;
    if path.exists() {
        if backup_path.exists() {
            std::fs::remove_file(&backup_path)?;
        }
        std::fs::rename(&path, &backup_path)?;
    }
    if let Err(error) = std::fs::rename(&temp_path, &path) {
        if backup_path.exists() {
            let _ = std::fs::rename(&backup_path, &path);
        }
        return Err(error.into());
    }
    if backup_path.exists() {
        std::fs::remove_file(backup_path)?;
    }
    Ok(())
}

fn read_settings(state: &State<'_, AppState>) -> Result<Settings, AppError> {
    state
        .settings
        .lock()
        .map(|settings| settings.clone())
        .map_err(|e| AppError::StateUnavailable(e.to_string()))
}

// ============================================================================
// P0 命令
// ============================================================================

/// 检测环境：Node.js / lark-cli / 飞书登录状态
#[tauri::command]
pub fn check_env() -> EnvStatus {
    env::check_env()
}

/// 自动安装 lark-cli
#[tauri::command]
pub fn setup_lark_cli() -> Result<String, AppError> {
    env::install_lark_cli()
}

/// 初始化飞书应用配置（阻塞模式，会打开浏览器）
#[tauri::command]
pub fn init_app(brand: String, lang: String) -> Result<String, AppError> {
    env::init_app_config(&brand, &lang)
}

/// 发起飞书登录（非阻塞模式）
///
/// 返回 DeviceInfo，前端拿到后打开浏览器
#[tauri::command]
pub fn start_login() -> Result<DeviceInfo, AppError> {
    env::start_login(&["docs", "drive", "wiki"])
}

/// 用 device_code 完成飞书登录（阻塞模式）
#[tauri::command]
pub fn complete_login(device_code: String) -> Result<LoginResult, AppError> {
    env::complete_login(&device_code)
}

/// 阻塞模式飞书登录（简化版，一步到位）
#[tauri::command]
pub fn login_feishu_blocking() -> Result<LoginResult, AppError> {
    // 先尝试非阻塞方式
    match env::start_login(&["docs", "drive", "wiki"]) {
        Ok(device_info) => {
            // 用 device_code 完成登录
            env::complete_login(&device_info.device_code)
        }
        Err(e) => {
            // 非阻塞模式失败，回退到阻塞模式
            match crate::lark::auth_login_blocking(&["docs", "drive", "wiki"]) {
                Ok(_) => {
                    let (identity, token_status, user_name) = crate::lark::whoami()?;
                    let success = !identity.is_empty()
                        && (token_status == "ready" || token_status == "needs_refresh");
                    Ok(LoginResult {
                        success,
                        user_name,
                        error: if success {
                            None
                        } else {
                            Some("登录后验证失败".to_string())
                        },
                    })
                }
                Err(e2) => Ok(LoginResult {
                    success: false,
                    user_name: None,
                    error: Some(format!("{}; {}", e, e2)),
                }),
            }
        }
    }
}

/// 预览文档：获取 Markdown 正文（不下载图片）
#[tauri::command]
pub fn preview_doc(url: String) -> Result<PreviewResult, AppError> {
    extract::preview_doc(&url)
}

/// 提取单篇文档（正文 + 图片下载 + 保存 .md）
#[tauri::command]
pub async fn extract_doc(
    url: String,
    output_dir: Option<String>,
    state: State<'_, AppState>,
) -> Result<ExtractResult, AppError> {
    let settings = read_settings(&state)?;
    let dir = output_dir.unwrap_or_else(|| settings.output_dir.clone());
    std::fs::create_dir_all(&dir)?;
    extract::extract_doc_async(&url, &dir, &settings).await
}

/// 获取当前设置
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, AppError> {
    read_settings(&state)
}

/// 保存设置
#[tauri::command]
pub fn set_settings(settings: Settings, state: State<'_, AppState>) -> Result<(), AppError> {
    save_settings(&settings)?;
    let mut current = state
        .settings
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    *current = settings;
    Ok(())
}

// ============================================================================
// P1 命令
// ============================================================================

/// 获取知识库目录树
#[tauri::command]
pub fn get_wiki_tree(wiki_url: String) -> Result<WikiNode, AppError> {
    wiki::get_wiki_tree(&wiki_url)
}

/// 批量提取知识库
///
/// - `wiki_url`: 知识库根节点链接
/// - `output_dir`: 输出目录（None 用默认）
/// - `selected_tokens`: 选中的节点 token 列表（None 表示全部提取）
#[tauri::command]
pub async fn extract_wiki(
    wiki_url: String,
    output_dir: Option<String>,
    selected_tokens: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<WikiExtractResult, AppError> {
    let settings = read_settings(&state)?;
    let dir = output_dir.unwrap_or_else(|| settings.output_dir.clone());
    std::fs::create_dir_all(&dir)?;
    wiki::extract_wiki(&wiki_url, &dir, &settings, selected_tokens.as_deref()).await
}

#[tauri::command]
pub fn get_progress(task_id: String, state: State<'_, AppState>) -> Result<Progress, AppError> {
    let tasks = state
        .tasks
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    let task = tasks
        .get(&task_id)
        .ok_or_else(|| AppError::InvalidInput("任务不存在".to_string()))?;
    task.progress
        .lock()
        .map(|p| p.clone())
        .map_err(|e| AppError::StateUnavailable(e.to_string()))
}

#[tauri::command]
pub fn cancel_task(task_id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let tasks = state
        .tasks
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    let task = tasks
        .get(&task_id)
        .ok_or_else(|| AppError::InvalidInput("任务不存在".to_string()))?;
    task.cancelled.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn start_extract_wiki(
    wiki_url: String,
    output_dir: Option<String>,
    selected_tokens: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let settings = read_settings(&state)?;
    let dir = output_dir.unwrap_or_else(|| settings.output_dir.clone());
    std::fs::create_dir_all(&dir)?;
    let task_id = Uuid::new_v4().to_string();
    let progress = Arc::new(Mutex::new(Progress::new(task_id.clone(), 0)));
    let cancelled = Arc::new(AtomicBool::new(false));
    state
        .tasks
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?
        .insert(
            task_id.clone(),
            TaskControl {
                progress: progress.clone(),
                cancelled: cancelled.clone(),
            },
        );
    let tasks = state.tasks.clone();
    let task_id_for_run = task_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(mut p) = progress.lock() {
            p.status = TaskStatus::Running;
        }
        let result = crate::wiki::extract_wiki_controlled(
            &wiki_url,
            &dir,
            &settings,
            selected_tokens.as_deref(),
            Some(progress.clone()),
            Some(cancelled.clone()),
        )
        .await;
        if let Ok(mut p) = progress.lock() {
            if cancelled.load(Ordering::Relaxed) {
                p.status = TaskStatus::Cancelled;
            } else if result.is_ok() {
                p.status = TaskStatus::Completed;
            } else {
                p.status = TaskStatus::Failed;
                p.errors
                    .push(result.err().map(|e| e.to_string()).unwrap_or_default());
            }
        }
        let _ = (tasks, task_id_for_run);
    });
    Ok(task_id)
}
