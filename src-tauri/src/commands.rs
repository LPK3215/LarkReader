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
    DeviceInfo, EnvStatus, ExtractResult, LoginResult, OutputPreflight, PreviewResult, Progress,
    Settings, SettingsStatus, TaskPhase, TaskStatus, WikiExtractResult, WikiNode, WikiTaskResult,
};
use crate::wiki;

/// 应用状态：设置（持久化到本地文件）
pub struct AppState {
    pub settings: Mutex<Settings>,
    pub tasks: Arc<Mutex<HashMap<String, TaskControl>>>,
    pub completed_tasks: Arc<Mutex<HashMap<String, WikiTaskResult>>>,
    pub settings_warning: Mutex<Option<String>>,
}

pub struct TaskControl {
    pub progress: Arc<Mutex<Progress>>,
    pub cancelled: Arc<AtomicBool>,
}

/// 配置文件路径
pub(crate) fn config_path() -> std::path::PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("LarkReader");
    std::fs::create_dir_all(&dir).ok();
    dir.join("settings.json")
}

pub(crate) fn history_path() -> std::path::PathBuf {
    config_path().with_file_name("task_history.json")
}

fn persist_task_history(tasks: &HashMap<String, WikiTaskResult>) {
    if let Ok(content) = serde_json::to_vec_pretty(tasks) {
        let path = history_path();
        let temp = path.with_extension("json.tmp");
        if std::fs::write(&temp, content).is_ok() {
            let backup = path.with_extension("json.bak");
            let had_existing = path.exists();
            if had_existing {
                let _ = std::fs::remove_file(&backup);
                if std::fs::rename(&path, &backup).is_err() {
                    let _ = std::fs::remove_file(&temp);
                    return;
                }
            }
            if std::fs::rename(&temp, &path).is_err() && had_existing {
                let _ = std::fs::rename(&backup, &path);
            } else {
                let _ = std::fs::remove_file(backup);
            }
        }
    }
}

pub(crate) fn clean_completed_tasks(tasks: &mut HashMap<String, WikiTaskResult>) {
    let now = chrono::Utc::now();
    tasks.retain(|_, task| {
        task.progress
            .finished_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .is_some_and(|finished| (now - finished.with_timezone(&chrono::Utc)).num_hours() < 24)
    });
    while tasks.len() >= 100 {
        let oldest = tasks
            .iter()
            .min_by_key(|(_, task)| task.progress.finished_at.clone())
            .map(|(id, _)| id.clone());
        if let Some(id) = oldest {
            tasks.remove(&id);
        } else {
            break;
        }
    }
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
    crate::models::validate_output_directory_writable(std::path::Path::new(&dir))
        .map_err(AppError::InvalidSetting)?;
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
    settings
        .validate_writable()
        .map_err(AppError::InvalidSetting)?;
    save_settings(&settings)?;
    let mut current = state
        .settings
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    *current = settings;
    *state
        .settings_warning
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))? = None;
    Ok(())
}

#[tauri::command]
pub fn get_settings_status(state: State<'_, AppState>) -> Result<SettingsStatus, AppError> {
    Ok(SettingsStatus {
        settings: read_settings(&state)?,
        warning: state
            .settings_warning
            .lock()
            .map_err(|e| AppError::StateUnavailable(e.to_string()))?
            .clone(),
    })
}

#[tauri::command]
pub fn preflight_output_dir(path: String) -> Result<OutputPreflight, AppError> {
    let path = std::path::PathBuf::from(path);
    crate::models::validate_output_directory_writable(&path).map_err(AppError::InvalidSetting)?;
    let available_bytes = fs2::available_space(&path)?;
    Ok(OutputPreflight {
        path: path.to_string_lossy().to_string(),
        writable: true,
        available_bytes,
    })
}

#[tauri::command]
pub fn open_output_dir(path: String) -> Result<(), AppError> {
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() || !path.is_dir() {
        return Err(AppError::InvalidInput(
            "输出目录不存在或不是绝对目录".to_string(),
        ));
    }
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|e| AppError::Other(format!("打开输出目录失败: {e}")))
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
    crate::models::validate_output_directory_writable(std::path::Path::new(&dir))
        .map_err(AppError::InvalidSetting)?;
    wiki::extract_wiki(&wiki_url, &dir, &settings, selected_tokens.as_deref()).await
}

#[tauri::command]
pub fn get_progress(task_id: String, state: State<'_, AppState>) -> Result<Progress, AppError> {
    if let Some(task) = state
        .tasks
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?
        .get(&task_id)
    {
        let mut progress = task
            .progress
            .lock()
            .map(|p| p.clone())
            .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
        progress.refresh_timing();
        return Ok(progress);
    }
    state
        .completed_tasks
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?
        .get(&task_id)
        .map(|task| task.progress.clone())
        .ok_or_else(|| AppError::InvalidInput("任务不存在".to_string()))
}

#[tauri::command]
pub fn get_task_result(
    task_id: String,
    state: State<'_, AppState>,
) -> Result<WikiTaskResult, AppError> {
    let completed = state
        .completed_tasks
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    completed
        .get(&task_id)
        .cloned()
        .ok_or_else(|| AppError::InvalidInput("任务尚未完成或不存在".to_string()))
}

#[tauri::command]
pub fn dismiss_task_result(task_id: String, state: State<'_, AppState>) -> Result<(), AppError> {
    let mut completed = state
        .completed_tasks
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    completed
        .remove(&task_id)
        .ok_or_else(|| AppError::InvalidInput("任务结果不存在".to_string()))?;
    persist_task_history(&completed);
    Ok(())
}

#[tauri::command]
pub fn list_task_history(state: State<'_, AppState>) -> Result<Vec<WikiTaskResult>, AppError> {
    let completed = state
        .completed_tasks
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    let mut values: Vec<_> = completed.values().cloned().collect();
    values.sort_by(|a, b| b.progress.finished_at.cmp(&a.progress.finished_at));
    Ok(values)
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
    crate::models::validate_output_directory_writable(std::path::Path::new(&dir))
        .map_err(AppError::InvalidSetting)?;
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
    let completed_tasks = state.completed_tasks.clone();
    let task_id_for_run = task_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(mut p) = progress.lock() {
            p.start_phase(TaskPhase::ScanningWiki);
        }
        let scan_url = wiki_url.clone();
        let tree_result =
            tauri::async_runtime::spawn_blocking(move || wiki::get_wiki_tree(&scan_url)).await;
        let result = match tree_result {
            Ok(Ok(tree)) => {
                if cancelled.load(Ordering::Relaxed) {
                    Err(AppError::Extract("任务已取消".to_string()))
                } else {
                    crate::wiki::extract_wiki_tree_controlled(
                        tree,
                        &dir,
                        &settings,
                        selected_tokens.as_deref(),
                        Some(progress.clone()),
                        Some(cancelled.clone()),
                    )
                    .await
                }
            }
            Ok(Err(error)) => Err(error),
            Err(error) => Err(AppError::Other(format!("知识库扫描任务异常: {error}"))),
        };
        if let Ok(mut p) = progress.lock() {
            if cancelled.load(Ordering::Relaxed) {
                p.finish(TaskStatus::Cancelled);
            } else if result.is_ok() {
                p.finish(TaskStatus::Completed);
            } else {
                if let Err(error) = &result {
                    p.errors.push(error.to_string());
                }
                p.finish(TaskStatus::Failed);
            }
        }
        let final_progress = progress
            .lock()
            .map(|p| p.clone())
            .unwrap_or_else(|_| Progress::new(task_id_for_run.clone(), 0));
        let task_result = match result {
            Ok(result) => WikiTaskResult {
                task_id: task_id_for_run.clone(),
                progress: final_progress,
                result: Some(result),
                error: None,
            },
            Err(error) => WikiTaskResult {
                task_id: task_id_for_run.clone(),
                progress: final_progress,
                result: None,
                error: Some(error.to_string()),
            },
        };
        if let Ok(mut completed) = completed_tasks.lock() {
            clean_completed_tasks(&mut completed);
            completed.insert(task_id_for_run.clone(), task_result);
            persist_task_history(&completed);
        }
        if let Ok(mut tasks) = tasks.lock() {
            tasks.remove(&task_id_for_run);
        }
    });
    Ok(task_id)
}
