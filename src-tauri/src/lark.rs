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
use crate::models::LarkResponse;

/// 需要清除的干扰环境变量
const ENV_TO_REMOVE: &[&str] = &["HERMES_HOME", "OPENCLAW_HOME", "LARK_CHANNEL"];

/// 构造一个已清理干扰环境变量的 lark-cli Command
///
/// Windows 上额外设置 CREATE_NO_WINDOW，避免每次调用都弹出 cmd.exe 黑框。
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

/// 带工作目录的执行入口。
///
/// lark-cli 1.0.93 对写类命令（media-preview / workbook-export / record-list 等）有
/// 输出路径白名单：只允许写入当前工作目录、系统临时目录或用户 home 下的 files 目录，
/// 其余绝对路径一律报 `unsafe output path`。因此在写文件前把子进程 cwd 设为
/// 输出目标的所在目录，使该目录本身成为白名单内的当前目录。
fn run_lark_in(
    args: &[&str],
    timeout: Duration,
    cancelled: Option<&AtomicBool>,
    current_dir: Option<&Path>,
) -> AppResult<String> {
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

            match (error_type, subtype) {
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

/// 登录申请的最小只读权限集（13 个，覆盖本项目全部业务命令）
///
/// 注意不要改回 `--domain docs/drive/wiki`：domain 是"大类目"，会捆绑申请
/// 95+ 个权限（大量写入类），实测 lark-cli 1.0.93 的 `--recommend` 几乎不起
/// 作用（101→95）。显式 `--scope` 才是精确申请（docs/LOGIN_ISSUE_20260905.md §3.1）。
///
/// 注意：显式申请≠最终授权范围。token 实际 scope 由开放平台应用后台已开通的
/// 权限点决定（向导创建的应用会把预置权限包一并授予，实测 13 申请 → 110+ 授权）。
/// 授权定稿：**只多不少、不做后台裁剪**（docs/FEISHU_AUTH.md §4.5）——本清单必须
/// 覆盖全部业务命令，漏一项 → 对应类导出必然失败；多授权不影响任何功能。
pub const LOGIN_SCOPES: &str = "docx:document:readonly docs:document.content:read \
     docs:document.media:download drive:file:download drive:drive.metadata:readonly \
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

pub fn sheets_export_controlled(
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
