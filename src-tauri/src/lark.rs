//! lark-cli 调用封装
//!
//! 直接参考 Python MVP (extract_generic.py) 的实现方式：
//! subprocess.run → json.loads → 检查 ok → 取 data
//! 不加多余的 --format json，不加 --overwrite，简单直接。

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use wait_timeout::ChildExt;

use crate::error::{AppError, AppResult};
use crate::models::{LarkResponse, ScopeCheck};

/// 需要清除的干扰环境变量
const ENV_TO_REMOVE: &[&str] = &["HERMES_HOME", "OPENCLAW_HOME", "LARK_CHANNEL"];

/// 构造一个已清理干扰环境变量的 lark-cli Command
///
/// Windows 上额外设置 CREATE_NO_WINDOW，避免每次调用都弹出 cmd.exe 黑框。
/// 在 PATH 各目录中解析命令的真实绝对路径（Windows 按 .cmd/.bat/.exe 顺序）。
///
/// 应用由不同方式启动时，子进程对无扩展名命令（npm / lark-cli）的解析依赖
/// 继承而来的 PATH，偶发 "program not found"。这里解析出绝对路径后显式传入，
/// 彻底规避该问题。找不到时原样返回（保持旧有报错行为）。
pub(crate) fn resolve_on_path(name: &str) -> std::path::PathBuf {
    let direct = std::path::PathBuf::from(name);
    if direct.is_file() {
        return direct;
    }
    let exts: &[&str] = if cfg!(windows) {
        &["cmd", "bat", "exe"]
    } else {
        &[""]
    };
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for ext in exts {
                let candidate = dir.join(if ext.is_empty() {
                    name.to_string()
                } else {
                    format!("{name}.{ext}")
                });
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    direct
}

fn build_command() -> Command {
    let lark_bin = if cfg!(windows) {
        "lark-cli.cmd"
    } else {
        "lark-cli"
    };
    let mut cmd = Command::new(resolve_on_path(lark_bin));
    for env in ENV_TO_REMOVE {
        cmd.env_remove(env);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW：禁止子进程创建可见控制台窗口
        cmd.creation_flags(0x08000000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // 让子进程成为独立进程组组长，超时/取消时才能按负 PID 整组终止
        // （见 kill_process_tree）。
        cmd.process_group(0);
    }
    cmd
}

/// 终止 lark-cli 进程及其整棵进程树。
///
/// 在 Windows 上 `lark-cli.cmd` 会被 std 以 cmd.exe 包装执行，cmd 再拉起
/// node(scripts/run.js) 与真正的 cli 进程；直接 `Child::kill()` 只杀掉
/// cmd 外壳，node 会变成孤儿继续在后台轮询——这正是 device-code 登录
/// 超时/取消后僵尸堆积的源头。无论系统工具是否成功，最后再兜底直接
/// kill 直接子进程。
///
/// - Windows：`taskkill /PID <pid> /T /F`（/T 终止整棵进程树）
/// - Unix：向进程组发 SIGKILL（负 PID，进程组号 == 组长 PID）
fn kill_process_tree(child: &mut std::process::Child) {
    let pid = child.id();
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .arg("-KILL")
            .arg(format!("-{}", pid))
            .status();
    }
    let _ = child.kill();
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

/// 从一行输出中提取第一个 http(s) 链接（大小写不敏感）。
///
/// 用于提取 `config init --new` 打印的浏览器创建向导 URL。输出可能带 ANSI
/// 颜色码与行尾标点，因此只截取 URL 合法字符段，并在结尾去掉可能混入的标点。
pub fn extract_first_url(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut lower = bytes.to_vec();
    for b in lower.iter_mut() {
        b.make_ascii_lowercase();
    }
    let mut i = 0;
    while i < lower.len() {
        let scheme_len = if lower[i..].starts_with(b"https://") {
            8
        } else if lower[i..].starts_with(b"http://") {
            7
        } else {
            i += 1;
            continue;
        };
        let start = i;
        let mut end = start + scheme_len;
        while end < bytes.len() {
            let c = bytes[end];
            // 只收 URL 合法字符；遇到引号/括号/空白/ANSI 转义等即停
            let ok = c.is_ascii_graphic()
                && !matches!(
                    c,
                    b'"' | b'\''
                        | b'`'
                        | b'<'
                        | b'>'
                        | b'\\'
                        | b'('
                        | b')'
                        | b'['
                        | b']'
                        | b'{'
                        | b'}'
                        | b','
                        | b';'
                );
            if !ok {
                break;
            }
            end += 1;
        }
        // 去掉结尾常见的标点/斜杠
        let mut trimmed_end = end;
        while trimmed_end > start + scheme_len
            && matches!(bytes[trimmed_end - 1], b'.' | b'/' | b'?' | b'#' | b':')
        {
            trimmed_end -= 1;
        }
        if trimmed_end > start + scheme_len {
            return Some(text[start..trimmed_end].to_string());
        }
        i = end;
    }
    None
}

/// 执行 lark-cli 命令，返回 stdout 字符串
///
/// - 自动清除 HERMES_HOME 等干扰变量
/// - 检查退出码，非零则报错
/// - 退出码为 0 时检查 JSON 的 ok 字段
fn run_lark_with_timeout(
    args: &[&str],
    timeout: Duration,
    cancelled: Option<&AtomicBool>,
) -> AppResult<String> {
    run_lark_in(args, timeout, cancelled, None)
}

/// lark-cli 的原始执行结果：不做 `ok` / 退出码判定，交给调用方决定怎么解释。
///
/// 存在的理由：授权范围巡检要读 `ok:false` 响应体里的 `missing_scopes`，
/// 而严格入口会把它转成一句话错误、丢掉结构。见 `parse_scope_check`。
struct RawRun {
    success: bool,
    stdout: String,
    stderr: String,
}

/// 带工作目录的执行入口（原始输出版）。
///
/// lark-cli 1.0.93 对写类命令（media-preview / workbook-export / record-list 等）有
/// 输出路径白名单：只允许写入当前工作目录、系统临时目录或用户 home 下的 files 目录，
/// 其余绝对路径一律报 `unsafe output path`。因此在写文件前把子进程 cwd 设为
/// 输出目标的所在目录，使该目录本身成为白名单内的当前目录。
fn run_lark_raw_in(
    args: &[&str],
    timeout: Duration,
    cancelled: Option<&AtomicBool>,
    current_dir: Option<&Path>,
) -> AppResult<RawRun> {
    let mut command = build_command();
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }
    let mut child = command
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
    let started = Instant::now();
    let status: ExitStatus = loop {
        if cancelled.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            kill_process_tree(&mut child);
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(AppError::Extract("任务已取消".to_string()));
        }
        if let Some(status) = child
            .wait_timeout(Duration::from_millis(200))
            .map_err(AppError::Io)?
        {
            break status;
        }
        if started.elapsed() >= timeout {
            kill_process_tree(&mut child);
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err(AppError::CommandTimeout(timeout.as_secs()));
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| AppError::Other("读取 stdout 的线程异常退出".to_string()))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| AppError::Other("读取 stderr 的线程异常退出".to_string()))??;
    let stdout = String::from_utf8_lossy(&stdout).to_string();
    let stderr = String::from_utf8_lossy(&stderr).to_string();

    Ok(RawRun {
        success: status.success(),
        stdout,
        stderr,
    })
}

/// 严格执行入口：非零退出码与 `ok:false` 一律转成中文错误。
fn run_lark_in(
    args: &[&str],
    timeout: Duration,
    cancelled: Option<&AtomicBool>,
    current_dir: Option<&Path>,
) -> AppResult<String> {
    let raw = run_lark_raw_in(args, timeout, cancelled, current_dir)?;

    if !raw.success {
        // 非零退出码：尝试从 stdout 和 stderr 中解析 JSON 错误
        for text in [&raw.stdout, &raw.stderr] {
            let json_str = extract_json(text);
            if let Ok(resp) = serde_json::from_str::<LarkResponse>(json_str) {
                if !resp.ok {
                    let err_msg = format_lark_error(&resp.error, resp.code);
                    return Err(AppError::LarkCliResponse(err_msg));
                }
            }
        }
        // JSON 解析失败，返回原始错误
        let msg = if !raw.stderr.is_empty() {
            &raw.stderr
        } else {
            &raw.stdout
        };
        return Err(AppError::LarkCliError(msg.trim().to_string()));
    }

    // 退出码为 0 时也要检查 ok 字段（lark-cli 有时退出码 0 但 ok=false）
    let json_str = extract_json(&raw.stdout);
    if let Ok(resp) = serde_json::from_str::<LarkResponse>(json_str) {
        if !resp.ok {
            let err_msg = format_lark_error(&resp.error, resp.code);
            return Err(AppError::LarkCliResponse(err_msg));
        }
    }

    Ok(raw.stdout)
}

pub fn run_lark(args: &[&str]) -> AppResult<String> {
    run_lark_with_timeout(args, Duration::from_secs(120), None)
}

/// 取输出路径的父目录作为写类命令的工作目录（仅当该目录真实存在时）。
///
/// 目录不存在时返回 None，退化为不设置 cwd（保持旧行为）。
fn write_dir_of(output_path: &str) -> Option<PathBuf> {
    let parent = Path::new(output_path).parent()?;
    if parent.as_os_str().is_empty() || parent.as_os_str() == "." {
        None
    } else if parent.is_dir() {
        Some(parent.to_path_buf())
    } else {
        None
    }
}

fn run_lark_quick(args: &[&str]) -> AppResult<String> {
    run_lark_with_timeout(args, Duration::from_secs(15), None)
}

fn run_lark_interactive(args: &[&str]) -> AppResult<String> {
    run_lark_with_timeout(args, Duration::from_secs(600), None)
}

/// 执行 lark-cli 命令，解析 JSON，返回 data 字段
///
/// 对应 Python: data = json.loads(result.stdout); data["data"]
pub fn run_lark_get_data(args: &[&str]) -> AppResult<serde_json::Value> {
    let mut last_error = None;
    for attempt in 0..3 {
        match run_lark(args).and_then(|stdout| {
            let json_str = extract_json(&stdout);
            let resp: LarkResponse = serde_json::from_str(json_str)
                .map_err(|e| AppError::JsonParse(format!("JSON 解析失败: {}", e)))?;
            resp.data
                .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))
        }) {
            Ok(data) => return Ok(data),
            Err(error) if attempt < 2 && is_retryable_cli_error(&error) => {
                last_error = Some(error);
                std::thread::sleep(Duration::from_secs(1 << attempt));
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| AppError::Other("命令重试失败".to_string())))
}

fn is_retryable_cli_error(error: &AppError) -> bool {
    match error {
        AppError::CommandTimeout(_) | AppError::Http(_) => true,
        AppError::LarkCliError(message) | AppError::LarkCliResponse(message) => {
            let message = message.to_ascii_lowercase();
            message.contains("network")
                || message.contains("connection")
                || message.contains("rate")
                || message.contains("too many")
                || message.contains("temporar")
                || message.contains("网络")
                || message.contains("频繁")
        }
        _ => false,
    }
}

/// 将 lark-cli 错误转换为用户友好的中文提示
fn format_lark_error(error: &Option<serde_json::Value>, code: Option<i32>) -> String {
    match error {
        Some(serde_json::Value::String(s)) => translate_error_message(s),
        Some(serde_json::Value::Object(obj)) => {
            let error_type = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let subtype = obj.get("subtype").and_then(|v| v.as_str()).unwrap_or("");
            let message = obj.get("message").and_then(|v| v.as_str()).unwrap_or("");

            let base = match (error_type, subtype) {
                ("authentication", "token_missing") | ("authentication", "token_expired") => {
                    "飞书未登录或登录已过期，请重新登录飞书账号。".to_string()
                }
                ("authentication", _) => {
                    format!("飞书认证失败：{}，请重新登录。", message)
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
            };
            // 权限类错误会额外带 missing_scopes / console_url，
            // 这是用户唯一的可操作出口，必须一并透出。
            with_permission_hints(base, obj)
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

/// 把 lark-cli 权限错误里的可操作信息追加到提示末尾。
///
/// 按 2026-10-09 的实测区分三类（同一句英文 message，处置完全不同，不能混为一谈）：
///
/// | 错误码 | 含义 | 用户该做什么 |
/// |---|---|---|
/// | `99991679` | 缺 OpenAPI scope（应用授权层） | 按 `missing_scopes` / `console_url` 补齐后重新登录 |
/// | `1069902` | **文档级**不允许导出/下载，或该账号只有阅读权限 | 与应用授权无关！需文档所有者放开导出，或改用有权限的账号 |
/// | `1063002` | 连文档的权限设置都无权读 → 当前身份不是所有者/管理者 | 用所有者账号操作，或请所有者授权 |
///
/// 另外无论哪类都补出 `错误码` 与 `log_id`：这两项是官方排查入口（服务端日志/工单/
/// troubleshooter URL）的必需参数，此前被丢弃，用户与维护者都只能靠猜。
fn with_permission_hints(base: String, obj: &serde_json::Map<String, serde_json::Value>) -> String {
    let missing: Vec<String> = obj
        .get("missing_scopes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let console_url = obj.get("console_url").and_then(|v| v.as_str());
    let code = obj.get("code").and_then(|v| v.as_i64());
    let log_id = obj.get("log_id").and_then(|v| v.as_str());

    let mut out = base;
    match code {
        Some(1069902) => out.push_str(
            "\n这是**文档级**限制：当前账号读得到该文档，但该文档不允许导出 / 下载\
             （或者你只有阅读权限）。请让文档所有者在「⋯ → 权限设置」里放开导出 / 下载，\
             或改用有权限的账号。它与应用授权（scope）无关，补权限也不会生效。",
        ),
        Some(1063002) => out.push_str(
            "\n当前账号连该文档的权限设置都无权读取：多半你不是文档的所有者 / 管理者。\
             请用所有者账号导出，或让所有者给你放开权限。",
        ),
        Some(99991679) => {
            out.push_str("\n这是缺少 OpenAPI 权限（scope），按下面提示补齐后重新登录即可。")
        }
        _ => {}
    }

    if !missing.is_empty() {
        out.push_str(&format!(
            "\n缺少的权限：{}。请在飞书开放平台该应用的「权限管理」中开通后重新登录。",
            missing.join("、")
        ));
    }
    if let Some(url) = console_url {
        out.push_str(&format!("\n权限配置链接：{url}"));
    }
    if code.is_some() || log_id.is_some() {
        out.push_str(&format!(
            "\n（错误码 {}，log_id {}）——进一步排查时把这两项给飞书开放平台即可定位到服务端日志。",
            code.map(|c| c.to_string())
                .unwrap_or_else(|| "-".to_string()),
            log_id.unwrap_or("-")
        ));
    }
    out
}

/// 翻译常见的英文错误消息为中文
fn translate_error_message(msg: &str) -> String {
    let msg = msg.trim();
    if msg.contains("hermes context")
        || msg.contains("OPENCLAW_HOME")
        || msg.contains("HERMES_HOME")
    {
        "检测到本机的 AI 工具环境变量（HERMES_HOME 等）干扰了 lark-cli。\
         请关闭相关 AI 工具后重启本应用再试。"
            .to_string()
    } else if msg.contains("authorization_pending") || msg.contains("slow_down") {
        "等待你在浏览器中完成授权。若授权页\"开通并授权\"点击无反应，\
         请换一个浏览器或无痕窗口重试（浏览器缓存/扩展可能导致提交静默失败）。"
            .to_string()
    } else if msg.contains("expired_token") {
        "授权链接已过期（有效期 10 分钟），请重新发起登录。".to_string()
    } else if msg.contains("access_denied") {
        "你在授权页拒绝或取消了授权，请重新登录。".to_string()
    } else if msg.contains("need_user_authorization") || msg.contains("token_missing") {
        "飞书未登录或登录已过期，请重新登录飞书账号。".to_string()
    } else if msg.contains("not found") || msg.contains("not exist") || msg.contains("Invalid") {
        "文档不存在或链接无效，请检查链接是否正确。".to_string()
    } else if msg.contains("rate_limit") || msg.contains("too many") {
        "请求过于频繁，请稍后再试。".to_string()
    } else if msg.contains("network") || msg.contains("connection") {
        "网络连接失败，请检查网络后重试。".to_string()
    } else if msg.contains("lacks permission")
        || msg.contains("no permission")
        || msg.contains("permission denied")
        || msg.contains("insufficient permission")
        || msg.contains("forbidden")
    {
        "飞书拒绝了本次请求。表格 / 多维表格导出失败常见两类原因：\
         ① 应用授权缺「导出云文档」权限点（docs:document:export，等价 drive:export:readonly）；\
         ② **文档本身**不允许导出 / 下载，或你的账号只有阅读权限（服务端错误码 1069902）——\
         这一类与应用授权无关，需请文档所有者放开导出权限，或改用有权限的账号。"
            .to_string()
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
    // 必须显式 --as user：不指定时 lark-cli 走 auto_detect，而 app 自身可取
    // bot token，auto 会选中 bot（identity="bot", tokenStatus="ready"），
    // 导致后端按 identity=="user" 判定登录时，用户已授权仍被认为未登录。
    // 本项目所有业务命令（docs/sheets/drive/base）都以 --as user 身份执行，
    // 登录状态检测必须与之保持一致。
    let stdout = run_lark_quick(&["whoami", "--as", "user"])?;
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

// ============================================================================
// 授权范围巡检（auth check）
// ============================================================================

/// `LOGIN_SCOPES` 的数组形式（常量字符串仍是唯一来源）
pub fn required_scopes() -> Vec<String> {
    LOGIN_SCOPES
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

/// 巡检当前 token 是否具备全部必需 scope。
///
/// 执行 `lark-cli auth check --scope <LOGIN_SCOPES> --json`——这是 lark-cli 专门用来
/// 「查当前 token 有没有这些 scope」的入口，不发业务请求、不改任何状态。
///
/// 与业务命令的关系：业务命令失败后用户只能看到一句 `user lacks permission ...`，
/// 巡检把这件事提前到体检阶段，并给出可执行的修复入口（清除登录态并重新登录）。
/// 任何执行/解析层面的意外都降级为 `unknown`——授权巡检本身不该把用户挡在功能外面。
pub fn check_scopes() -> ScopeCheck {
    let required = required_scopes();
    if !lark_cli_exists() {
        return ScopeCheck::skipped("lark-cli 未安装，跳过授权范围检查");
    }
    let scope_arg = required.join(" ");
    let raw = match run_lark_raw_in(
        &["auth", "check", "--scope", &scope_arg, "--json"],
        Duration::from_secs(20),
        None,
        None,
    ) {
        Ok(raw) => raw,
        Err(error) => return ScopeCheck::unknown(required, format!("授权范围检查失败：{error}")),
    };
    // 失败时 lark-cli 可能把 JSON 打到 stderr（stdout 空）
    let text = if raw.stdout.trim().is_empty() {
        raw.stderr.as_str()
    } else {
        raw.stdout.as_str()
    };
    parse_scope_check(text, &required)
}

/// 解析 `auth check --json` 的输出。
///
/// **保守判定**：只有拿到确切的缺项证据（已授予清单逐项比对、或响应里直接给缺项清单）
/// 才返回 `missing`；其余无法判定的一律 `unknown`，不阻断导出。
/// 1.0.93 成功响应的字段名无法离线确证，所以把几种常见命名都兜住。
fn parse_scope_check(text: &str, required: &[String]) -> ScopeCheck {
    let clean = strip_ansi(text);
    let json_str = extract_json(&clean);
    let value: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(value) => value,
        Err(error) => {
            return ScopeCheck::unknown(
                required.to_vec(),
                format!("无法解析权限检查结果（{error}）"),
            )
        }
    };

    let error = value.get("error");

    // **未登录必须先分流**：实测 `auth check` 在无令牌时返回
    // `{"error": "not_logged_in", "missing": [<全部申请项>], "ok": false}` ——
    // 这里的 missing 是"根本没有令牌"的连带结果，不是"令牌缺权限"。
    // 若照字面判成 missing，未登录会被当成"缺 14 项权限"拦下导出，文案完全指错方向。
    let error_code = error
        .and_then(|e| {
            e.as_str()
                .map(str::to_string)
                .or_else(|| {
                    e.get("subtype")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .or_else(|| e.get("type").and_then(|v| v.as_str()).map(str::to_string))
        })
        .unwrap_or_default();
    if matches!(
        error_code.as_str(),
        "not_logged_in"
            | "token_missing"
            | "token_expired"
            | "need_user_authorization"
            | "authentication"
    ) {
        return ScopeCheck::skipped("未登录或登录已过期，暂不检查授权范围");
    }

    let granted = json_string_array(
        &value,
        &[
            "granted",
            "granted_scopes",
            "grantedScopes",
            "scopes_granted",
        ],
    )
    .filter(|list| !list.is_empty())
    .or_else(|| {
        value.get("data").and_then(|data| {
            json_string_array(data, &["granted", "granted_scopes", "scopes_granted"])
                .filter(|list| !list.is_empty())
        })
    });

    let missing_explicit = error
        .and_then(|e| json_string_array(e, &["missing_scopes", "missingScopes", "missing"]))
        .filter(|list| !list.is_empty())
        .or_else(|| {
            json_string_array(&value, &["missing", "missing_scopes", "missingScopes"])
                .filter(|list| !list.is_empty())
        });

    // 1) 有已授予清单：逐项比对（最可靠）
    if let Some(granted) = granted {
        let missing: Vec<String> = required
            .iter()
            .filter(|scope| !granted.contains(*scope))
            .cloned()
            .collect();
        if missing.is_empty() {
            return ScopeCheck::ok(
                required.to_vec(),
                granted.clone(),
                format!(
                    "已授权 {} 项，覆盖全部 {} 项必需权限",
                    granted.len(),
                    required.len()
                ),
            );
        }
        return ScopeCheck::missing(
            required.to_vec(),
            granted,
            missing.clone(),
            format!(
                "缺少 {} 项授权：{}。需清除本机登录态并重新登录后才能补齐。",
                missing.len(),
                missing.join("、")
            ),
        );
    }

    // 2) 响应里直接给了缺项清单
    if let Some(missing) = missing_explicit {
        return ScopeCheck::missing(
            required.to_vec(),
            Vec::new(),
            missing.clone(),
            format!(
                "缺少 {} 项授权：{}。需清除本机登录态并重新登录后才能补齐。",
                missing.len(),
                missing.join("、")
            ),
        );
    }

    // 3) 命令自报通过：认为必需 scope 齐全
    if value.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return ScopeCheck::ok(
            required.to_vec(),
            Vec::new(),
            format!("权限检查通过（{} 项必需权限齐全）", required.len()),
        );
    }

    // 4) 其余降级为 unknown，并尽量带上命令自己的解释
    let detail = error
        .and_then(|e| e.get("message"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| clean.trim().chars().take(200).collect::<String>());
    let detail = detail.trim();
    ScopeCheck::unknown(
        required.to_vec(),
        if detail.is_empty() {
            "授权范围检查未返回可用结果".to_string()
        } else {
            format!("无法确认授权范围：{}", translate_error_message(detail))
        },
    )
}

/// 从 JSON 对象里按候选键名取字符串数组（数组为空视作没有）
fn json_string_array(value: &serde_json::Value, keys: &[&str]) -> Option<Vec<String>> {
    for key in keys {
        if let Some(array) = value.get(*key).and_then(|v| v.as_array()) {
            return Some(
                array
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
            );
        }
    }
    None
}

/// 去掉 ANSI 颜色转义（部分终端下 lark-cli 会给 JSON 上色）
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // ESC [ ... <字母>：整段控制序列丢弃
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// 后台流式执行 `config init --new`（阻塞式浏览器创建向导）。
///
/// 这是创建飞书应用的**唯一**入口：同步阻塞版会占住调用方最长 600 秒且拿不到
/// 向导 URL，已移除。本函数把每一行 stdout/stderr 实时交给 `on_line`，
/// 供调用方在向导阻塞期间提取验证 URL 并持续更新进度。命令最长运行 600 秒，
/// 超时/异常退出均返回 Err，正常退出返回最后一行 stdout（去空行）。
pub fn config_init_stream(
    brand: &str,
    lang: &str,
    on_line: Arc<dyn Fn(&str) + Send + Sync>,
) -> AppResult<String> {
    let args = [
        "config".to_string(),
        "init".to_string(),
        "--new".to_string(),
        "--brand".to_string(),
        brand.to_string(),
        "--lang".to_string(),
        lang.to_string(),
    ];
    let mut command = build_command();
    let mut child = command
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AppError::LarkCliNotFound(e.to_string()))?;
    let stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Other("无法读取 lark-cli stdout".to_string()))?;
    let stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Other("无法读取 lark-cli stderr".to_string()))?;

    let cb_out = on_line.clone();
    let out_reader = std::thread::spawn(move || {
        let reader = BufReader::new(stdout_pipe);
        let mut last = String::new();
        for line in reader.lines() {
            let Ok(line) = line else { break };
            cb_out(&line);
            if !line.trim().is_empty() {
                last = line;
            }
        }
        last
    });
    let cb_err = on_line.clone();
    let err_reader = std::thread::spawn(move || {
        let reader = BufReader::new(stderr_pipe);
        let mut last = String::new();
        for line in reader.lines() {
            let Ok(line) = line else { break };
            cb_err(&line);
            if !line.trim().is_empty() {
                last = line;
            }
        }
        last
    });

    const TIMEOUT_SECS: u64 = 600;
    let started = Instant::now();
    let status: ExitStatus = loop {
        if let Some(status) = child
            .wait_timeout(Duration::from_millis(200))
            .map_err(AppError::Io)?
        {
            break status;
        }
        if started.elapsed() >= Duration::from_secs(TIMEOUT_SECS) {
            kill_process_tree(&mut child);
            let _ = child.wait();
            let _ = out_reader.join();
            let _ = err_reader.join();
            return Err(AppError::CommandTimeout(TIMEOUT_SECS));
        }
    };
    let out_last = out_reader
        .join()
        .map_err(|_| AppError::Other("读取 stdout 的线程异常退出".to_string()))?;
    let err_last = err_reader
        .join()
        .map_err(|_| AppError::Other("读取 stderr 的线程异常退出".to_string()))?;

    if status.success() {
        Ok(out_last)
    } else {
        let msg = if !err_last.trim().is_empty() {
            err_last
        } else {
            out_last
        };
        Err(AppError::LarkCliError(msg.trim().to_string()))
    }
}

/// 登录申请的最小只读权限集（14 个，覆盖本项目全部业务命令）
///
/// 注意不要改回 `--domain docs/drive/wiki`：domain 是"大类目"，会捆绑申请
/// 95+ 个权限（大量写入类），实测 lark-cli 1.0.93 的 `--recommend` 几乎不起
/// 作用（101→95）。显式 `--scope` 才是精确申请（docs/LOGIN_ISSUE_20260905.md §3.1）。
///
/// 注意：显式申请≠最终授权范围。token 实际 scope 由开放平台应用后台已开通的
/// 权限点决定（向导创建的应用会把预置权限包一并授予，实测 14 申请 → 110+ 授权）。
/// 授权定稿：**只多不少、不做后台裁剪**（docs/FEISHU_AUTH.md §4.5）——本清单必须
/// 覆盖全部业务命令，漏一项 → 对应类导出必然失败；多授权不影响任何功能。
///
/// `docs:document:export`（2026-10-08 补）：`sheets +workbook-export` 内部走飞书
/// 「创建导出任务」API（`POST /open-apis/drive/v1/export_tasks`，可用
/// `sheets +workbook-export --dry-run` 看到），该接口的「权限要求」是
/// **导出云文档**——`docs:document:export` 与 `drive:export:readonly` 二者任选其一。
/// `sheets:spreadsheet:read` 只够读表格结构与数据，**不能**替代导出权限。
/// 本清单此前只有 sheets 那条、唯独缺导出权限，导致表格导出对未额外开通该权限
/// 的应用必然失败（报 `user lacks permission for the requested resource`）。
///
/// 取 `docs:document:export` 而非等价项，依据是本机实测：`lark-cli auth check` 显示
/// 作者本机 token `granted: [docs:document:export]` 而缺 `drive:export:readonly`，
/// 同一台机器上表格导出可用——它才是本项目已验证可行的那一项。
pub const LOGIN_SCOPES: &str = "docx:document:readonly docs:document.content:read \
     docs:document.media:download docs:document:export \
     drive:file:download drive:drive.metadata:readonly \
     wiki:node:read wiki:node:retrieve wiki:space:retrieve \
     sheets:spreadsheet:read base:app:read base:table:read base:record:read base:field:read";

/// 执行 `lark-cli auth login --scope <LOGIN_SCOPES>`（阻塞模式）
pub fn auth_login_blocking() -> AppResult<String> {
    run_lark_interactive(&["auth", "login", "--scope", LOGIN_SCOPES])
}

/// 执行 `lark-cli auth login --scope <LOGIN_SCOPES> --no-wait --json`（非阻塞模式）
pub fn auth_login_no_wait() -> AppResult<String> {
    run_lark(&[
        "auth",
        "login",
        "--scope",
        LOGIN_SCOPES,
        "--no-wait",
        "--json",
    ])
}

/// 执行 `lark-cli auth login --device-code <code>`
///
/// lark-cli 自述最长阻塞约 10 分钟等待用户在浏览器完成授权；
/// 超时上限 620s 略大于 lark-cli 内部上限，避免恰好临界误杀。
/// 注意：该命令不可并发/重启执行——lark-cli 每次重启会作废上一轮的 device code。
pub fn auth_login_with_device_code(device_code: &str) -> AppResult<String> {
    run_lark_with_timeout(
        &["auth", "login", "--device-code", device_code],
        Duration::from_secs(620),
        None,
    )
}

/// 执行 `lark-cli auth logout --json`
///
/// 清除 lark-cli 保存的飞书登录凭据（token）。退出后 whoami / check_env
/// 将返回未登录状态。普通写命令，短超时即可。
pub fn auth_logout() -> AppResult<String> {
    run_lark(&["auth", "logout", "--json"])
}

/// 执行 `lark-cli docs +fetch --doc <url> --doc-format markdown --as user`
///
/// 对应 Python: fetch_doc(node_token)
/// 返回文档的 Markdown 正文
pub fn docs_fetch(url: &str) -> AppResult<String> {
    docs_fetch_controlled(url, None)
}

pub fn docs_fetch_controlled(url: &str, cancelled: Option<&AtomicBool>) -> AppResult<String> {
    let stdout = run_lark_with_timeout(
        &[
            "docs",
            "+fetch",
            "--doc",
            url,
            "--doc-format",
            "markdown",
            "--as",
            "user",
        ],
        Duration::from_secs(120),
        cancelled,
    )?;
    let resp: LarkResponse = serde_json::from_str(extract_json(&stdout))
        .map_err(|e| AppError::JsonParse(e.to_string()))?;
    let data = resp
        .data
        .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))?;

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
    docs_media_preview_controlled(token, output_path, None)
}

pub fn docs_media_preview_controlled(
    token: &str,
    output_path: &str,
    cancelled: Option<&AtomicBool>,
) -> AppResult<String> {
    let stdout = run_lark_in(
        &[
            "docs",
            "+media-preview",
            "--token",
            token,
            "--output",
            output_path,
            "--as",
            "user",
        ],
        Duration::from_secs(120),
        cancelled,
        write_dir_of(output_path).as_deref(),
    )?;
    let resp: LarkResponse = serde_json::from_str(extract_json(&stdout))
        .map_err(|e| AppError::JsonParse(e.to_string()))?;
    let data = resp
        .data
        .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))?;

    let media_data: crate::models::MediaPreviewData =
        serde_json::from_value(data).map_err(|e| AppError::JsonParse(e.to_string()))?;

    media_data
        .saved_path
        .ok_or_else(|| AppError::LarkCliResponse("media-preview 返回中缺少 saved_path".to_string()))
}

pub fn sheets_export(url: &str, output_path: &str) -> AppResult<String> {
    sheets_export_controlled(url, output_path, None)
}

/// 导出电子表格：先走官方「导出任务」接口，被拒时降级为"读单元格 + 本地生成 xlsx"。
///
/// 为什么要降级（2026-10-09 实测）：导出任务接口（`POST /open-apis/drive/v1/export_tasks`）
/// 除了应用授权，还要求**文档本身允许导出/下载**。只读协作者拿到的是
/// `1069902 permission_denied`——而"只能读、但不能导出"恰恰是本工具最典型的用户场景。
/// 实测同一条文档：`+workbook-info` / `+csv-get` 这类**读接口**在只读身份下完全可用，
/// 所以被拒时改用读接口取数据、本地拼 xlsx，做到"只读文档也能导出"。
///
/// 取舍：降级产物只有**值**（公式以计算结果落盘），没有原表格的样式/合并/图表；
/// 官方导出能成功时永远优先官方导出。
pub fn sheets_export_controlled(
    url: &str,
    output_path: &str,
    cancelled: Option<&AtomicBool>,
) -> AppResult<String> {
    match sheets_export_via_task_api(url, output_path, cancelled) {
        Ok(saved) => Ok(saved),
        Err(error) if is_export_permission_denied(&error) => {
            crate::logger::info(format!(
                "导出任务接口被拒（{error}），降级为读单元格生成 xlsx：{url}"
            ));
            sheets_export_via_read_api(url, output_path)
        }
        Err(error) => Err(error),
    }
}

/// 官方导出任务接口（`sheets +workbook-export`）
fn sheets_export_via_task_api(
    url: &str,
    output_path: &str,
    cancelled: Option<&AtomicBool>,
) -> AppResult<String> {
    let stdout = run_lark_in(
        &[
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
        ],
        Duration::from_secs(120),
        cancelled,
        write_dir_of(output_path).as_deref(),
    )?;
    let resp: LarkResponse = serde_json::from_str(extract_json(&stdout))
        .map_err(|e| AppError::JsonParse(e.to_string()))?;
    let data = resp
        .data
        .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))?;
    Ok(data
        .get("saved_path")
        .or_else(|| data.get("output_path"))
        .and_then(|value| value.as_str())
        .unwrap_or(output_path)
        .to_string())
}

/// 是否为"文档级不让导出"类错误（服务端 1069902 / 权限被拒）。
///
/// 只有这一类才降级：网络故障、限频、token 过期等应当原样报错，
/// 不能因为读接口恰好还能用就把真实问题藏起来。
fn is_export_permission_denied(error: &AppError) -> bool {
    let text = error.to_string();
    text.contains("1069902")
        || text.contains("lacks permission")
        || text.contains("permission_denied")
        || text.contains("不允许导出")
}

/// 降级导出：`+workbook-info` 取子表清单 → 每个子表 `+csv-get` 取数据 → 本地写 xlsx。
fn sheets_export_via_read_api(url: &str, output_path: &str) -> AppResult<String> {
    let (title, sheets) = sheets_workbook_info(url)?;
    if sheets.is_empty() {
        return Err(AppError::LarkCliResponse(format!(
            "表格「{title}」没有可读取的子表"
        )));
    }

    let mut book = rust_xlsxwriter::Workbook::new();
    for (sheet_id, sheet_name) in &sheets {
        let csv_text = sheets_csv_text(url, sheet_id)?;
        let worksheet = book.add_worksheet();
        let name = sanitize_sheet_name(sheet_name);
        worksheet
            .set_name(name)
            .map_err(|e| AppError::Other(format!("设置工作表名失败: {e}")))?;

        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(csv_text.as_bytes());
        for (row, record) in reader.records().enumerate() {
            let record = record.map_err(|e| AppError::Other(format!("解析 CSV 失败: {e}")))?;
            for (col, field) in record.iter().enumerate() {
                if field.is_empty() {
                    continue;
                }
                worksheet
                    .write_string(row as u32, col as u16, field)
                    .map_err(|e| AppError::Other(format!("写入单元格失败: {e}")))?;
            }
        }
    }

    book.save(output_path)
        .map_err(|e| AppError::Other(format!("写出 xlsx 失败: {e}")))?;
    Ok(output_path.to_string())
}

/// xlsx 工作表名限制：≤31 字符，且不能含 `[ ] : * ? / \`。
fn sanitize_sheet_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\') {
                '_'
            } else {
                c
            }
        })
        .take(31)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Sheet".to_string()
    } else {
        trimmed.to_string()
    }
}

/// 读电子表格结构：返回 `(标题, [(sheet_id, sheet_name)])`。
fn sheets_workbook_info(url: &str) -> AppResult<(String, Vec<(String, String)>)> {
    let stdout = run_lark(&[
        "sheets",
        "+workbook-info",
        "--url",
        url,
        "--as",
        "user",
        "--format",
        "json",
    ])?;
    let resp: LarkResponse = serde_json::from_str(extract_json(&stdout))
        .map_err(|e| AppError::JsonParse(e.to_string()))?;
    let data = resp
        .data
        .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))?;

    let title = data
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let sheets = data
        .get("sheets")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let id = item.get("sheet_id").and_then(|v| v.as_str())?;
                    let name = item
                        .get("sheet_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or(id);
                    Some((id.to_string(), name.to_string()))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok((title, sheets))
}

/// 读单个子表为干净 CSV 文本。
///
/// `+csv-get` 默认返回带 `[row=N] ` 行号前缀的 `annotated_csv`（方便人看），
/// 这里剥掉前缀得到可直接解析的 CSV；若接口已给纯 `csv` 字段则优先用它。
fn sheets_csv_text(url: &str, sheet_id: &str) -> AppResult<String> {
    let stdout = run_lark(&[
        "sheets",
        "+csv-get",
        "--url",
        url,
        "--sheet-id",
        sheet_id,
        "--as",
        "user",
        "--format",
        "json",
    ])?;
    let resp: LarkResponse = serde_json::from_str(extract_json(&stdout))
        .map_err(|e| AppError::JsonParse(e.to_string()))?;
    let data = resp
        .data
        .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))?;

    if let Some(plain) = data.get("csv").and_then(|v| v.as_str()) {
        return Ok(plain.to_string());
    }
    let annotated = data
        .get("annotated_csv")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            AppError::LarkCliResponse("csv-get 返回里既没有 csv 也没有 annotated_csv".to_string())
        })?;
    Ok(strip_row_annotations(annotated))
}

/// 去掉 `[row=N] ` 行号前缀
fn strip_row_annotations(text: &str) -> String {
    text.lines()
        .map(|line| {
            line.strip_prefix('[')
                .and_then(|rest| rest.find("] ").map(|idx| &rest[idx + 2..]))
                .unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 执行 `lark-cli drive +preview --file-token <token> --type source_file --output <path> --as user`
///
/// 用于下载挂载在 Wiki 上的普通文件附件（zip/pdf 等，obj_type=file）。
/// 注意不能用 `drive +download`：它对非可导出类型报
/// “current identity does not have export permission for this Drive file”，
/// 需改用 `+preview --type source_file` 直接取原文件。
/// 返回本地保存路径。
pub fn drive_file_preview(token: &str, output_path: &str) -> AppResult<String> {
    drive_file_preview_controlled(token, output_path, None)
}

pub fn drive_file_preview_controlled(
    token: &str,
    output_path: &str,
    cancelled: Option<&AtomicBool>,
) -> AppResult<String> {
    let stdout = run_lark_in(
        &[
            "drive",
            "+preview",
            "--file-token",
            token,
            "--type",
            "source_file",
            "--output",
            output_path,
            "--as",
            "user",
        ],
        Duration::from_secs(300),
        cancelled,
        write_dir_of(output_path).as_deref(),
    )?;
    let resp: LarkResponse = serde_json::from_str(extract_json(&stdout))
        .map_err(|e| AppError::JsonParse(e.to_string()))?;
    let data = resp
        .data
        .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))?;
    Ok(data
        .get("output_path")
        .or_else(|| data.get("saved_path"))
        .and_then(|value| value.as_str())
        .unwrap_or(output_path)
        .to_string())
}

pub fn base_table_list(base_token: &str) -> AppResult<serde_json::Value> {
    base_table_list_controlled(base_token, None)
}

pub fn base_table_list_controlled(
    base_token: &str,
    cancelled: Option<&AtomicBool>,
) -> AppResult<serde_json::Value> {
    let stdout = run_lark_with_timeout(
        &[
            "base",
            "+table-list",
            "--base-token",
            base_token,
            "--as",
            "user",
        ],
        Duration::from_secs(120),
        cancelled,
    )?;
    let resp: LarkResponse = serde_json::from_str(extract_json(&stdout))
        .map_err(|e| AppError::JsonParse(e.to_string()))?;
    resp.data
        .ok_or_else(|| AppError::LarkCliResponse("响应中缺少 data 字段".to_string()))
}

pub fn base_records_export(
    base_token: &str,
    table_id: &str,
    output_path: &str,
) -> AppResult<String> {
    base_records_export_controlled(base_token, table_id, output_path, None)
}

pub fn base_records_export_controlled(
    base_token: &str,
    table_id: &str,
    output_path: &str,
    cancelled: Option<&AtomicBool>,
) -> AppResult<String> {
    run_lark_in(
        &[
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
        ],
        Duration::from_secs(120),
        cancelled,
        write_dir_of(output_path).as_deref(),
    )?;
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

/// 执行 `lark-cli wiki +node-list --space-id <id> --page-all --as user --format json`
///
/// 不带 `--parent-node-token`，返回该 space 下的**全部顶层节点**（互为兄弟）。
/// 用于 FullSpace 模式（整库展开）：用户传入的 URL 可能指向一个无子节点的文档，
/// 此时通过 space 级 node-list 获取所有顶层节点，逐个递归即可覆盖整个知识库。
pub fn wiki_space_roots(space_id: &str) -> AppResult<Vec<crate::models::NodeListItem>> {
    let data = run_lark_get_data(&[
        "wiki",
        "+node-list",
        "--space-id",
        space_id,
        "--page-all",
        "--as",
        "user",
        "--format",
        "json",
    ])?;

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

#[cfg(test)]
mod url_tests {
    use super::extract_first_url;
    #[test]
    fn extracts_plain_https() {
        assert_eq!(
            extract_first_url("Go to https://open.feishu.cn/app/new"),
            Some("https://open.feishu.cn/app/new".to_string())
        );
    }

    #[test]
    fn strips_ansi_and_trailing_punctuation() {
        assert_eq!(
            extract_first_url("\u{1b}[36mhttps://example.com/a?b=1&c=2\u{1b}[0m，请打开"),
            Some("https://example.com/a?b=1&c=2".to_string())
        );
    }

    #[test]
    fn http_and_uppercase() {
        assert_eq!(
            extract_first_url("HTTP://A.B/x"),
            Some("HTTP://A.B/x".to_string())
        );
    }

    #[test]
    fn none_when_no_link() {
        assert_eq!(extract_first_url("正在等待创建…"), None);
    }

    #[test]
    fn url_after_chinese_text() {
        assert_eq!(
            extract_first_url(
                "请在浏览器中打开以下链接：https://open.feishu.cn/app/create，完成创建"
            ),
            Some("https://open.feishu.cn/app/create".to_string())
        );
    }
}

#[cfg(test)]
mod sheets_fallback_tests {
    use super::*;

    /// 端到端验证「只读文档降级导出」：需要真实网络 + 本机已登录，因此默认 `#[ignore]`。
    ///
    /// 目标文档是 E2E 测试库里的一张**只读**电子表格——导出任务接口对它返回
    /// `1069902`（文档级不允许导出），正是本降级路径要覆盖的场景。
    ///
    /// 手动运行：`cargo test --lib sheets_read_fallback -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn sheets_read_fallback_writes_xlsx() {
        let out = std::env::temp_dir().join("larkreader-fallback-test.xlsx");
        let _ = std::fs::remove_file(&out);

        let saved = sheets_export_controlled(
            "https://qcny2iztd1p8.feishu.cn/wiki/IeqYwAakGisB05kIqJqcEcB5nJe",
            &out.to_string_lossy(),
            None,
        )
        .expect("只读文档也应当能导出");

        let size = std::fs::metadata(&saved).expect("产物应存在").len();
        assert!(size > 1000, "xlsx 过小，数据可能没写进去：{size} 字节");
        println!("降级导出产物：{saved}（{size} 字节）");
    }

    #[test]
    fn strips_row_annotation_prefix() {
        assert_eq!(
            strip_row_annotations("[row=1] 姓名,部门\n[row=2] 张三,技术部"),
            "姓名,部门\n张三,技术部"
        );
    }

    #[test]
    fn sanitizes_sheet_name() {
        assert_eq!(sanitize_sheet_name("Sheet1"), "Sheet1");
        assert_eq!(sanitize_sheet_name("a/b:c*d?e[f]g"), "a_b_c_d_e_f_g");
        assert_eq!(sanitize_sheet_name("   "), "Sheet");
        assert_eq!(sanitize_sheet_name(&"x".repeat(40)).len(), 31);
    }

    #[test]
    fn detects_export_permission_denied() {
        assert!(is_export_permission_denied(&AppError::LarkCliResponse(
            "飞书拒绝了本次请求（错误码 1069902）".to_string()
        )));
        assert!(is_export_permission_denied(&AppError::LarkCliResponse(
            "user lacks permission for the requested resource".to_string()
        )));
        assert!(!is_export_permission_denied(&AppError::LarkCliResponse(
            "请求过于频繁，请稍后再试。".to_string()
        )));
    }
}

#[cfg(test)]
mod scope_tests {
    use super::{parse_scope_check, strip_ansi, LOGIN_SCOPES};
    use crate::models::ScopeCheck;

    fn required() -> Vec<String> {
        LOGIN_SCOPES
            .split_whitespace()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn strips_ansi_colors() {
        assert_eq!(strip_ansi("\u{1b}[31;1m{\u{1b}[0m"), "{");
    }

    #[test]
    fn ok_when_command_reports_success() {
        let check = parse_scope_check(r#"{"ok": true}"#, &required());
        assert_eq!(check.state, "ok");
        assert!(check.missing.is_empty());
    }

    #[test]
    fn ok_when_granted_covers_all_required() {
        let mut granted = required();
        granted.push("im:message".to_string()); // 多授权不影响判定
        let json = format!(r#"{{"ok": true, "granted": {}}}"#, to_json(&granted));
        let check = parse_scope_check(&json, &required());
        assert_eq!(check.state, "ok");
        assert_eq!(check.granted.len(), required().len() + 1);
    }

    #[test]
    fn missing_derived_from_granted_list() {
        let mut granted = required();
        granted.retain(|scope| scope != "docs:document:export");
        let json = format!(r#"{{"ok": true, "granted": {}}}"#, to_json(&granted));
        let check = parse_scope_check(&json, &required());
        assert_eq!(check.state, "missing");
        assert_eq!(check.missing, vec!["docs:document:export".to_string()]);
        assert!(check.message.contains("docs:document:export"));
    }

    #[test]
    fn missing_from_error_envelope() {
        let raw = "\u{1b}[31;1m{\"ok\": false, \"error\": {\"type\": \"authorization\", \
                   \"subtype\": \"missing_scope\", \"message\": \"...\", \
                   \"missing_scopes\": [\"docs:document:export\"]}}\u{1b}[0m";
        let check = parse_scope_check(raw, &required());
        assert_eq!(check.state, "missing");
        assert_eq!(check.missing, vec!["docs:document:export".to_string()]);
    }

    /// 实测输出（本机 1.0.93，未登录）：missing 列的是"全部申请项"，
    /// 属"没有令牌"的连带结果，必须判 skipped，绝不能当作缺权限拦导出。
    #[test]
    fn not_logged_in_is_skipped_not_missing() {
        let raw = r#"{
          "_notice": {"update": {"current": "1.0.93", "latest": "1.0.97"}},
          "error": "not_logged_in",
          "missing": ["docx:document:readonly", "docs:document:export"],
          "ok": false
        }"#;
        let check = parse_scope_check(raw, &required());
        assert_eq!(check.state, "skipped");
        assert!(check.missing.is_empty());
    }

    /// 未登录时若带的是结构化 error 对象（另一种可能形态），同样不能判成 missing
    #[test]
    fn authentication_error_object_is_skipped() {
        let raw = r#"{"ok": false, "error": {"type": "authentication", "subtype": "token_expired"}, "missing": ["docs:document:export"]}"#;
        let check = parse_scope_check(raw, &required());
        assert_eq!(check.state, "skipped");
    }

    /// 已登录但确实缺项：顶层 missing 直接给出，判 missing
    #[test]
    fn logged_in_with_top_level_missing() {
        let raw = r#"{"ok": false, "error": "missing_scope", "missing": ["docs:document:export"]}"#;
        let check = parse_scope_check(raw, &required());
        assert_eq!(check.state, "missing");
        assert_eq!(check.missing, vec!["docs:document:export".to_string()]);
    }

    #[test]
    fn unknown_on_unparsable_output() {
        let check = parse_scope_check("boom", &required());
        assert_eq!(check.state, "unknown");
        assert!(check.missing.is_empty());
    }

    #[test]
    fn unknown_on_not_configured_error() {
        let raw = r#"{"ok": false, "error": {"type": "config", "subtype": "not_configured", "message": "not configured"}}"#;
        let check = parse_scope_check(raw, &required());
        assert_eq!(check.state, "unknown");
        assert!(check.message.contains("not configured"));
    }

    #[test]
    fn check_helper_defaults_to_unknown_state_string() {
        // 未登录场景不得被判成 ok（前端按 state == "missing" 才拦截，ok 会误导展示）
        let skipped: ScopeCheck = ScopeCheck::skipped("未登录");
        assert_eq!(skipped.state, "skipped");
        assert!(skipped.missing.is_empty());
    }

    fn to_json(scopes: &[String]) -> String {
        let quoted: Vec<String> = scopes.iter().map(|s| format!("\"{s}\"")).collect();
        format!("[{}]", quoted.join(", "))
    }
}
