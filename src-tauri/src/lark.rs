//! lark-cli 调用封装
//!
//! 直接参考 Python MVP (extract_generic.py) 的实现方式：
//! subprocess.run → json.loads → 检查 ok → 取 data
//! 不加多余的 --format json，不加 --overwrite，简单直接。

use std::io::Read;
use std::process::{Command, ExitStatus, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

use crate::error::{AppError, AppResult};
use crate::models::LarkResponse;

/// 需要清除的干扰环境变量
const ENV_TO_REMOVE: &[&str] = &["HERMES_HOME", "OPENCLAW_HOME", "LARK_CHANNEL"];

/// 构造一个已清理干扰环境变量的 lark-cli Command
fn build_command() -> Command {
    let lark_bin = if cfg!(windows) {
        "lark-cli.cmd"
    } else {
        "lark-cli"
    };
    let mut cmd = Command::new(lark_bin);
    for env in ENV_TO_REMOVE {
        cmd.env_remove(env);
    }
    cmd
}

/// 从输出字符串中提取 JSON 部分
///
/// lark-cli 有时在 JSON 前输出日志行如 `[lark-cli] xxx`，
/// 找到第一个 `{` 开始截取
fn extract_json(stdout: &str) -> &str {
    let trimmed = stdout.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return trimmed;
    }
    if let Some(pos) = trimmed.find('{') {
        return &trimmed[pos..];
    }
    trimmed
}

/// 执行 lark-cli 命令，返回 stdout 字符串
///
/// - 自动清除 HERMES_HOME 等干扰变量
/// - 检查退出码，非零则报错
/// - 退出码为 0 时检查 JSON 的 ok 字段
fn run_lark_with_timeout(args: &[&str], timeout: Duration) -> AppResult<String> {
    let mut child = build_command()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::LarkCliNotFound(e.to_string()))?;
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Other("无法读取 lark-cli stdout".to_string()))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Other("无法读取 lark-cli stderr".to_string()))?;
    let stdout_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout_pipe.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr_pipe.read_to_end(&mut bytes).map(|_| bytes)
    });
    let status: ExitStatus =
        if let Some(status) = child.wait_timeout(timeout).map_err(AppError::Io)? {
            status
        } else {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(AppError::CommandTimeout(timeout.as_secs()));
        };
    let stdout = stdout_reader
        .join()
        .map_err(|_| AppError::Other("读取 stdout 的线程异常退出".to_string()))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| AppError::Other("读取 stderr 的线程异常退出".to_string()))??;
    let stdout = String::from_utf8_lossy(&stdout).to_string();
    let stderr = String::from_utf8_lossy(&stderr).to_string();

    if !status.success() {
        // 非零退出码：尝试从 stdout 和 stderr 中解析 JSON 错误
        for text in [&stdout, &stderr] {
            let json_str = extract_json(text);
            if let Ok(resp) = serde_json::from_str::<LarkResponse>(json_str) {
                if !resp.ok {
                    let err_msg = format_lark_error(&resp.error, resp.code);
                    return Err(AppError::LarkCliResponse(err_msg));
                }
            }
        }
        // JSON 解析失败，返回原始错误
        let msg = if !stderr.is_empty() { &stderr } else { &stdout };
        return Err(AppError::LarkCliError(msg.trim().to_string()));
    }

    // 退出码为 0 时也要检查 ok 字段（lark-cli 有时退出码 0 但 ok=false）
    let json_str = extract_json(&stdout);
    if let Ok(resp) = serde_json::from_str::<LarkResponse>(json_str) {
        if !resp.ok {
            let err_msg = format_lark_error(&resp.error, resp.code);
            return Err(AppError::LarkCliResponse(err_msg));
        }
    }

    Ok(stdout)
}

pub fn run_lark(args: &[&str]) -> AppResult<String> {
    run_lark_with_timeout(args, Duration::from_secs(120))
}

fn run_lark_quick(args: &[&str]) -> AppResult<String> {
    run_lark_with_timeout(args, Duration::from_secs(15))
}

fn run_lark_interactive(args: &[&str]) -> AppResult<String> {
    run_lark_with_timeout(args, Duration::from_secs(600))
}

/// 执行 lark-cli 命令，解析 JSON，返回 data 字段
///
/// 对应 Python: data = json.loads(result.stdout); data["data"]
pub fn run_lark_get_data(args: &[&str]) -> AppResult<serde_json::Value> {
    let stdout = run_lark(args)?;
    let json_str = extract_json(&stdout);
    let resp: LarkResponse = serde_json::from_str(json_str)
        .map_err(|e| AppError::JsonParse(format!("JSON 解析失败: {}", e)))?;
    resp.data
        .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))
}

/// 将 lark-cli 错误转换为用户友好的中文提示
fn format_lark_error(error: &Option<serde_json::Value>, code: Option<i32>) -> String {
    match error {
        Some(serde_json::Value::String(s)) => translate_error_message(s),
        Some(serde_json::Value::Object(obj)) => {
            let error_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let subtype = obj.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
            let message = obj.get("message").and_then(|v| v.as_str()).unwrap_or("");

            match (error_type, subtype) {
                ("authentication", "token_missing") | ("authentication", "token_expired") => {
                    "飞书未登录或登录已过期，请重新登录飞书账号。".to_string()
                }
                ("authentication", _) => {
                    format!("飞书认证失败：{}，请重新登录。", message)
                }
                ("permission", _) => {
                    format!(
                        "权限不足：{}，请确认你的飞书账号有该文档的阅读权限。",
                        message
                    )
                }
                ("rate_limit", _) => "请求过于频繁，请稍后再试。".to_string(),
                ("not_found", _) => "文档不存在或链接无效，请检查链接是否正确。".to_string(),
                _ => {
                    if !message.is_empty() {
                        translate_error_message(message)
                    } else if let Some(c) = code {
                        format!("操作失败，错误码：{}", c)
                    } else {
                        "操作失败，请稍后重试。".to_string()
                    }
                }
            }
        }
        _ => {
            if let Some(c) = code {
                format!("操作失败，错误码：{}", c)
            } else {
                "操作失败，原因未知。".to_string()
            }
        }
    }
}

/// 翻译常见的英文错误消息为中文
fn translate_error_message(msg: &str) -> String {
    let msg = msg.trim();
    if msg.contains("need_user_authorization") || msg.contains("token_missing") {
        "飞书未登录或登录已过期，请重新登录飞书账号。".to_string()
    } else if msg.contains("permission_denied") || msg.contains("permission") {
        format!("权限不足：{}，请确认你的飞书账号有该文档的阅读权限。", msg)
    } else if msg.contains("not found") || msg.contains("not exist") || msg.contains("Invalid") {
        "文档不存在或链接无效，请检查链接是否正确。".to_string()
    } else if msg.contains("rate_limit") || msg.contains("too many") {
        "请求过于频繁，请稍后再试。".to_string()
    } else if msg.contains("network") || msg.contains("connection") {
        "网络连接失败，请检查网络后重试。".to_string()
    } else {
        msg.to_string()
    }
}

// ============================================================================
// 具体命令封装 — 直接对应 Python 参考代码的命令参数
// ============================================================================

/// 执行 `lark-cli whoami`
///
/// 返回 (identity, token_status, user_name)
pub fn whoami() -> AppResult<(String, String, Option<String>)> {
    let stdout = run_lark_quick(&["whoami"])?;
    let json_str = extract_json(&stdout);
    let resp: crate::models::WhoamiResponse =
        serde_json::from_str(json_str).map_err(|e| AppError::JsonParse(e.to_string()))?;

    let identity = resp.identity.unwrap_or_default();
    let token_status = resp.token_status.unwrap_or_default();
    let user_name = resp.on_behalf_of.and_then(|o| o.user_name);

    Ok((identity, token_status, user_name))
}

/// 执行 `lark-cli config show`
///
/// 返回 (app_id, brand)，如果未配置返回 None
pub fn config_show() -> AppResult<Option<(String, String)>> {
    let stdout = run_lark_quick(&["config", "show"])?;
    let json_str = extract_json(&stdout);
    let resp: crate::models::ConfigResponse =
        serde_json::from_str(json_str).map_err(|e| AppError::JsonParse(e.to_string()))?;

    Ok(resp
        .app_id
        .map(|app_id| (app_id, resp.brand.unwrap_or_default())))
}

/// 执行 `lark-cli config init --new --brand feishu --lang zh`
pub fn config_init(brand: &str, lang: &str) -> AppResult<String> {
    run_lark_interactive(&["config", "init", "--new", "--brand", brand, "--lang", lang])
}

/// 执行 `lark-cli auth login --domain docs --domain drive --domain wiki`（阻塞模式）
pub fn auth_login_blocking(domains: &[&str]) -> AppResult<String> {
    let mut args = vec!["auth", "login"];
    for d in domains {
        args.push("--domain");
        args.push(d);
    }
    run_lark_interactive(&args)
}

/// 执行 `lark-cli auth login --no-wait --json`（非阻塞模式）
pub fn auth_login_no_wait(domains: &[&str]) -> AppResult<String> {
    let mut args = vec!["auth", "login"];
    for d in domains {
        args.push("--domain");
        args.push(d);
    }
    args.push("--no-wait");
    args.push("--json");
    run_lark(&args)
}

/// 执行 `lark-cli auth login --device-code <code>`
pub fn auth_login_with_device_code(device_code: &str) -> AppResult<String> {
    run_lark_interactive(&["auth", "login", "--device-code", device_code])
}

/// 执行 `lark-cli docs +fetch --doc <url> --doc-format markdown --as user`
///
/// 对应 Python: fetch_doc(node_token)
/// 返回文档的 Markdown 正文
pub fn docs_fetch(url: &str) -> AppResult<String> {
    let data = run_lark_get_data(&[
        "docs",
        "+fetch",
        "--doc",
        url,
        "--doc-format",
        "markdown",
        "--as",
        "user",
    ])?;

    // 解析 data.document.content
    let fetch_data: crate::models::FetchDocData =
        serde_json::from_value(data).map_err(|e| AppError::JsonParse(e.to_string()))?;

    Ok(fetch_data.document.content)
}

/// 执行 `lark-cli docs +media-preview --token <token> --output <path> --as user`
///
/// 对应 Python: preview_image(token, output_path)
/// 命令参数与 Python 参考代码完全一致：不加 --format json，不加 --overwrite
/// 返回图片保存路径
pub fn docs_media_preview(token: &str, output_path: &str) -> AppResult<String> {
    let data = run_lark_get_data(&[
        "docs",
        "+media-preview",
        "--token",
        token,
        "--output",
        output_path,
        "--as",
        "user",
    ])?;

    let media_data: crate::models::MediaPreviewData =
        serde_json::from_value(data).map_err(|e| AppError::JsonParse(e.to_string()))?;

    media_data
        .saved_path
        .ok_or_else(|| AppError::LarkCliResponse("media-preview 返回中缺少 saved_path".to_string()))
}

pub fn sheets_export(url: &str, output_path: &str) -> AppResult<String> {
    let data = run_lark_get_data(&[
        "sheets",
        "+workbook-export",
        "--url",
        url,
        "--file-extension",
        "xlsx",
        "--output-path",
        output_path,
        "--as",
        "user",
    ])?;
    Ok(data
        .get("saved_path")
        .or_else(|| data.get("output_path"))
        .and_then(|value| value.as_str())
        .unwrap_or(output_path)
        .to_string())
}

pub fn base_table_list(base_token: &str) -> AppResult<serde_json::Value> {
    run_lark_get_data(&[
        "base",
        "+table-list",
        "--base-token",
        base_token,
        "--as",
        "user",
    ])
}

pub fn base_records_export(
    base_token: &str,
    table_id: &str,
    output_path: &str,
) -> AppResult<String> {
    run_lark(&[
        "base",
        "+record-list",
        "--base-token",
        base_token,
        "--table-id",
        table_id,
        "--format",
        "ndjson",
        "--output",
        output_path,
        "--overwrite",
        "--as",
        "user",
    ])?;
    Ok(output_path.to_string())
}

/// 执行 `lark-cli wiki +node-get --node-token <token> --as user --format json`
///
/// 返回节点详情（space_id、obj_token、has_child 等）
pub fn wiki_node_get(node_token: &str) -> AppResult<crate::models::NodeGetInfo> {
    let data = run_lark_get_data(&[
        "wiki",
        "+node-get",
        "--node-token",
        node_token,
        "--as",
        "user",
        "--format",
        "json",
    ])?;

    serde_json::from_value(data).map_err(|e| AppError::JsonParse(e.to_string()))
}

/// 执行 `lark-cli wiki +node-list --space-id <id> --parent-node-token <token> --page-all --as user --format json`
///
/// 返回子节点列表
pub fn wiki_node_list(
    space_id: &str,
    parent_node_token: &str,
) -> AppResult<Vec<crate::models::NodeListItem>> {
    let data = run_lark_get_data(&[
        "wiki",
        "+node-list",
        "--space-id",
        space_id,
        "--parent-node-token",
        parent_node_token,
        "--page-all",
        "--as",
        "user",
        "--format",
        "json",
    ])?;

    // node-list 返回格式: { "has_more": false, "nodes": [...] }
    if data.is_array() {
        serde_json::from_value(data).map_err(|e| AppError::JsonParse(e.to_string()))
    } else if let Some(items) = data.get("nodes") {
        serde_json::from_value(items.clone()).map_err(|e| AppError::JsonParse(e.to_string()))
    } else if let Some(items) = data.get("items") {
        serde_json::from_value(items.clone()).map_err(|e| AppError::JsonParse(e.to_string()))
    } else if data.is_null() {
        Ok(vec![])
    } else {
        serde_json::from_value(data).map_err(|e| AppError::JsonParse(e.to_string()))
    }
}

/// 执行 `lark-cli --version`
pub fn lark_cli_version() -> AppResult<String> {
    let output = build_command()
        .arg("--version")
        .output()
        .map_err(|e| AppError::LarkCliNotFound(e.to_string()))?;

    if !output.status.success() {
        return Err(AppError::LarkCliError("无法获取 lark-cli 版本".to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 检查 lark-cli 是否可执行
pub fn lark_cli_exists() -> bool {
    build_command()
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
