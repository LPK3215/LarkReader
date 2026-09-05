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
use crate::lark;
use crate::models::{
    AppInitStatus, DeviceInfo, EnvStatus, ExportableCount, LogFileContent, LogFileMeta,
    LoginResult, OutputPreflight, Progress, ReaderBinary, ReaderEntry, Settings, SettingsStatus,
    TaskPhase, TaskStatus, WikiNode, WikiTaskResult,
};
use crate::wiki;

/// 应用状态：设置（持久化到本地文件）
pub struct AppState {
    pub settings: Mutex<Settings>,
    pub tasks: Arc<Mutex<HashMap<String, TaskControl>>>,
    pub completed_tasks: Arc<Mutex<HashMap<String, WikiTaskResult>>>,
    pub settings_warning: Mutex<Option<String>>,
    /// 飞书应用创建向导状态（start_app_init / get_app_init_status）
    pub app_init: Arc<Mutex<AppInitStatus>>,
    /// 最近一次扫描得到的知识库树（wiki URL + 树）。
    ///
    /// 前端扫树后进入勾选，`count_exportable` 与 `start_extract_wiki` 都直接复用，
    /// 避免同一棵树在"预览"和"开始下载"时被重复扫描两次（大库扫描耗时可观）。
    pub last_tree: Arc<Mutex<Option<(String, WikiNode)>>>,
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

/// 把登录结果写进运行日志
fn log_login_result(result: &LoginResult) {
    if result.success {
        crate::logger::info(format!(
            "飞书登录成功：{}",
            result.user_name.as_deref().unwrap_or("未知用户")
        ));
    } else {
        crate::logger::warn(format!(
            "飞书登录失败：{}",
            result.error.as_deref().unwrap_or("未知错误")
        ));
    }
}

/// 长文本截断（用于日志，避免单行过长）
fn clip_for_log(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let mut clipped: String = text.chars().take(max_chars).collect();
        clipped.push('…');
        clipped
    }
}

// ============================================================================
// P0 命令
// ============================================================================

/// 检测环境：Node.js / lark-cli / 飞书登录状态
#[tauri::command]
pub async fn check_env() -> EnvStatus {
    tauri::async_runtime::spawn_blocking(env::check_env)
        .await
        .unwrap_or_else(|e| {
            crate::logger::error(format!("环境检测任务异常: {e}"));
            EnvStatus::default()
        })
}

/// 自动安装 lark-cli
#[tauri::command]
pub async fn setup_lark_cli() -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(env::install_lark_cli)
        .await
        .map_err(|e| AppError::Other(format!("lark-cli 安装任务异常: {e}")))?
}

/// 在后台启动飞书应用创建向导（`config init --new`）并立即返回。
///
/// lark-cli 的 `config init --new` 是阻塞式浏览器向导：命令在后台运行，逐行打印
/// 输出（含验证 URL）。本命令把 stdout/stderr 逐行流式转发到 `app_init` 状态，
/// 前端通过轮询 `get_app_init_status` 拿到 `url` 后自动打开浏览器。
///
/// 不能并发发起两个向导（lark-cli 每次只支持一个待完成的创建流程）。
#[tauri::command]
pub async fn start_app_init(
    state: State<'_, AppState>,
    brand: String,
    lang: String,
) -> Result<AppInitStatus, AppError> {
    let status = state.app_init.clone();
    {
        let cur = status
            .lock()
            .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
        if cur.running {
            return Err(AppError::Other(
                "已有应用创建流程正在运行，请先在浏览器完成或稍后重试".to_string(),
            ));
        }
    }
    {
        let mut s = status
            .lock()
            .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
        *s = AppInitStatus {
            running: true,
            stage: "正在启动飞书应用创建向导…".to_string(),
            url: None,
            message: Some("正在调用 lark-cli…".to_string()),
            error: None,
        };
    }
    crate::logger::info(format!(
        "开始飞书应用创建向导（config init --new，brand={brand}，lang={lang}）"
    ));

    let cb_status = status.clone();
    let on_line: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |line: &str| {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Ok(mut s) = cb_status.lock() {
            // 裁剪到 300 字，避免把超长行整个塞进状态
            let clipped: String = trimmed.chars().take(300).collect();
            s.message = Some(clipped);
            if s.url.is_none() {
                if let Some(url) = lark::extract_first_url(trimmed) {
                    s.url = Some(url);
                    s.stage = "授权链接已生成，正在自动打开浏览器…".to_string();
                }
            }
        }
    });

    let run_status = status.clone();
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            lark::config_init_stream(&brand, &lang, on_line)
        })
        .await;
        if let Ok(mut s) = run_status.lock() {
            s.running = false;
            match result {
                Ok(Ok(last_line)) => {
                    s.stage = "已完成".to_string();
                    let tail = last_line.trim();
                    s.message = Some(if tail.is_empty() {
                        "应用创建成功，配置已写入 lark-cli".to_string()
                    } else {
                        format!("应用创建成功：{tail}")
                    });
                    crate::logger::info("飞书应用创建向导完成");
                }
                Ok(Err(err)) => {
                    s.stage = "失败".to_string();
                    s.error = Some(err.to_string());
                    crate::logger::error(format!("飞书应用创建失败：{err}"));
                }
                Err(err) => {
                    s.stage = "失败".to_string();
                    s.error = Some(format!("创建向导后台任务异常：{err}"));
                    crate::logger::error(format!("飞书应用创建向导异常：{err}"));
                }
            }
        }
    });

    let snapshot = status
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?
        .clone();
    Ok(snapshot)
}

/// 查询飞书应用创建向导的实时状态（轮询用）
#[tauri::command]
pub fn get_app_init_status(state: State<'_, AppState>) -> Result<AppInitStatus, AppError> {
    let s = state
        .app_init
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    Ok(s.clone())
}

/// 发起飞书登录（非阻塞模式）
///
/// 返回 DeviceInfo，前端拿到后打开浏览器
#[tauri::command]
pub async fn start_login() -> Result<DeviceInfo, AppError> {
    let device_info = tauri::async_runtime::spawn_blocking(env::start_login)
        .await
        .map_err(|e| AppError::Other(format!("登录发起任务异常: {e}")))??;
    crate::logger::info("发起飞书登录（设备码授权流程）");
    Ok(device_info)
}

/// 用 device_code 完成飞书登录（阻塞等待授权）
///
/// 内部运行 `lark-cli auth login --device-code <code>`，最长阻塞约 10 分钟
/// 直到用户在浏览器完成授权。运行在独立线程，不占用 IPC 串行队列。
/// 注意：不得并发发起多个该命令——lark-cli 每次重启会作废上一轮的 device code。
#[tauri::command]
pub async fn complete_login(device_code: String) -> Result<LoginResult, AppError> {
    let login = tauri::async_runtime::spawn_blocking(move || env::complete_login(&device_code))
        .await
        .map_err(|e| AppError::Other(format!("登录等待任务异常: {e}")))?;
    match &login {
        Ok(result) => log_login_result(result),
        Err(error) => crate::logger::error(format!("飞书登录异常：{error}")),
    }
    login
}

/// 退出飞书登录（清除 lark-cli 保存的 token）
#[tauri::command]
pub async fn logout() -> Result<String, AppError> {
    let result = tauri::async_runtime::spawn_blocking(env::logout)
        .await
        .map_err(|e| AppError::Other(format!("退出登录任务异常: {e}")))??;
    crate::logger::info("退出飞书登录");
    Ok(result)
}

/// 保存设置
#[tauri::command]
pub fn set_settings(settings: Settings, state: State<'_, AppState>) -> Result<(), AppError> {
    settings
        .validate_writable()
        .map_err(AppError::InvalidSetting)?;
    save_settings(&settings)?;
    crate::logger::info(format!(
        "更新设置：输出目录={}，图片并发数={}，下载图片={}",
        settings.output_dir, settings.concurrency, settings.download_images
    ));
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
///
/// 扫描结果会缓存到 `AppState.last_tree`，供后续 `count_exportable` 与
/// `start_extract_wiki` 复用，避免同一棵树被扫描两次。
///
/// `scan_mode`：可选，默认 `auto`（Auto 模式，只导出传入节点及其子树）。
/// 传 `full_space`（FullSpace 模式）时，如果传入节点无子节点，自动展开整个
/// 知识库（列出 space 下全部顶层节点）。不传或传 `auto` 时行为不变。
#[tauri::command]
pub async fn get_wiki_tree(
    wiki_url: String,
    scan_mode: Option<wiki::ScanMode>,
    state: State<'_, AppState>,
) -> Result<WikiNode, AppError> {
    let mode = scan_mode.unwrap_or_default();
    let tree = tauri::async_runtime::spawn_blocking({
        let wiki_url = wiki_url.clone();
        move || wiki::get_wiki_tree_with_mode(&wiki_url, mode)
    })
    .await
    .map_err(|e| AppError::Other(format!("扫描知识库任务异常: {e}")))??;
    if let Ok(mut cached) = state.last_tree.lock() {
        *cached = Some((wiki_url, tree.clone()));
    }
    crate::logger::info(format!(
        "扫描知识库「{}」（模式 {:?}）：{} 个顶层节点",
        tree.title,
        mode,
        tree.children.len()
    ));
    Ok(tree)
}

/// 统计勾选范围内真实会被导出的条目数（下载前的预估）
///
/// 勾选一个父节点会展开成它的全部可导出后代，因此这里返回的数字通常**大于**
/// 用户直接勾选的节点数。口径与任务进度里的 `total` 完全一致。
#[tauri::command]
pub fn count_exportable(
    selected_tokens: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<ExportableCount, AppError> {
    let cached = state
        .last_tree
        .lock()
        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
    let Some((_, tree)) = cached.as_ref() else {
        return Err(AppError::InvalidInput(
            "尚未扫描知识库，请先扫描目录结构".to_string(),
        ));
    };
    Ok(wiki::count_exportable_breakdown(
        tree,
        selected_tokens.as_deref(),
    ))
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
    scan_mode: Option<wiki::ScanMode>,
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
    crate::logger::info(format!(
        "开始导出任务 {task_id}：wiki={}，输出目录={}{}",
        clip_for_log(&wiki_url, 160),
        dir,
        selected_tokens
            .as_ref()
            .map(|tokens| format!("，勾选 {} 个节点", tokens.len()))
            .unwrap_or_default()
    ));
    // 若前端刚扫描过同一棵树，直接复用，省掉一次完整的目录遍历
    let cached_tree = match state.last_tree.lock() {
        Ok(cached) => match cached.as_ref() {
            Some((url, tree)) if *url == wiki_url => Some(tree.clone()),
            _ => None,
        },
        Err(_) => None,
    };
    let tasks = state.tasks.clone();
    let completed_tasks = state.completed_tasks.clone();
    let task_id_for_run = task_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(mut p) = progress.lock() {
            p.start_phase(TaskPhase::ScanningWiki);
        }
        let tree_result = match cached_tree {
            Some(tree) => {
                crate::logger::info("复用已扫描的知识库树，跳过重复扫描");
                Ok(Ok(tree))
            }
            None => {
                let scan_url = wiki_url.clone();
                let scan_mode = scan_mode.unwrap_or_default();
                tauri::async_runtime::spawn_blocking(move || {
                    wiki::get_wiki_tree_with_mode(&scan_url, scan_mode)
                })
                .await
            }
        };
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
        match &result {
            Ok(wiki_result) => crate::logger::info(format!(
                "导出任务 {task_id_for_run} 完成：成功 {}，失败 {}，部分 {}，跳过 {}（共 {} 项），用时 {} 秒，输出目录：{}",
                wiki_result.success_count,
                wiki_result.failed_count,
                wiki_result.partial_count,
                wiki_result.skipped_count,
                wiki_result.total,
                final_progress.elapsed_seconds,
                wiki_result.output_root
            )),
            Err(error) => {
                crate::logger::error(format!("导出任务 {task_id_for_run} 失败：{error}"))
            }
        }
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

// ============================================================================
// 运行日志
// ============================================================================

/// 单次读取日志文件的字节上限（超出只返回末尾，避免撑爆前端）
const MAX_LOG_READ_BYTES: u64 = 512 * 1024;

/// 列出日志目录里的所有日志文件（按名称倒序，最新在前）
#[tauri::command]
pub fn list_log_files() -> Result<Vec<LogFileMeta>, AppError> {
    let dir = crate::logger::log_dir();
    let mut metas = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("log") {
                continue;
            }
            let metadata = entry.metadata().ok();
            metas.push(LogFileMeta {
                name: entry.file_name().to_string_lossy().into_owned(),
                size_bytes: metadata.as_ref().map(|meta| meta.len()).unwrap_or(0),
                modified_at: metadata
                    .as_ref()
                    .and_then(|meta| meta.modified().ok())
                    .map(|time| chrono::DateTime::<chrono::Local>::from(time).to_rfc3339()),
            });
        }
    }
    metas.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(metas)
}

/// 读取指定日志文件的文本内容（过大时只返回末尾部分）
#[tauri::command]
pub fn read_log_file(name: String) -> Result<LogFileContent, AppError> {
    // 防止路径穿越：只允许纯文件名（不含目录分隔符 / ..）
    let file_name = std::path::Path::new(&name)
        .file_name()
        .and_then(|value| value.to_str());
    if file_name != Some(name.as_str()) || !name.ends_with(".log") {
        return Err(AppError::InvalidInput("非法的日志文件名".to_string()));
    }
    let path = crate::logger::log_dir().join(&name);
    let metadata = std::fs::metadata(&path)?;
    let size_bytes = metadata.len();
    let mut file = std::fs::File::open(&path)?;
    let (bytes, truncated) = if size_bytes > MAX_LOG_READ_BYTES {
        use std::io::{Read, Seek, SeekFrom};
        file.seek(SeekFrom::End(-(MAX_LOG_READ_BYTES as i64)))?;
        let mut buffer = vec![0u8; MAX_LOG_READ_BYTES as usize];
        file.read_exact(&mut buffer)?;
        (buffer, true)
    } else {
        use std::io::Read;
        let mut buffer = Vec::with_capacity(size_bytes as usize);
        file.read_to_end(&mut buffer)?;
        (buffer, false)
    };
    let content = String::from_utf8_lossy(&bytes).into_owned();
    Ok(LogFileContent {
        name,
        content,
        size_bytes,
        truncated,
    })
}

/// 打开日志目录（在文件管理器中查看日志文件）
#[tauri::command]
pub fn open_log_dir() -> Result<(), AppError> {
    let dir = crate::logger::log_dir();
    std::fs::create_dir_all(&dir)?;
    tauri_plugin_opener::open_path(dir, None::<&str>)
        .map_err(|e| AppError::Other(format!("打开日志目录失败: {e}")))
}

// ============================================================================
// 本地阅读（Reader）
// ============================================================================

/// 列出本地目录的一层子项（Reader 目录导航，惰性加载：目录优先、按名排序）
#[tauri::command]
pub fn list_reader_dir(path: String) -> Result<Vec<ReaderEntry>, AppError> {
    crate::reader::list_reader_dir(&path)
}

/// 读取 .md 文档文本（Reader 渲染正文）
#[tauri::command]
pub fn read_reader_md(path: String) -> Result<String, AppError> {
    crate::reader::read_reader_md(&path)
}

/// 读取二进制资源（图片等），返回可内联的 data URL
#[tauri::command]
pub fn read_reader_binary(path: String) -> Result<ReaderBinary, AppError> {
    crate::reader::read_reader_binary(&path)
}

/// 在导出目录树里找第一篇 Markdown（任务历史「应用内阅读」自动打开用）
#[tauri::command]
pub fn find_first_reader_doc(path: String) -> Result<Option<String>, AppError> {
    crate::reader::find_first_reader_doc(&path)
}
