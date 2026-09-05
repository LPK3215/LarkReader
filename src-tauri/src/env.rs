//! 环境检测与引导
//!
//! 职责：
//! 1. 检测 Node.js / lark-cli / 飞书登录状态
//! 2. 自动安装 lark-cli
//! 3. 引导用户飞书登录授权（非阻塞模式）

use std::process::Command;

use crate::error::{AppError, AppResult};
use crate::lark;
use crate::models::{DeviceInfo, EnvCheckError, EnvStatus, LoginResult};

const SUPPORTED_LARK_CLI_VERSION: &str = "1.0.93";

/// 检测 Node.js 是否安装，返回版本号
pub fn check_node() -> Option<String> {
    let output = Command::new("node").arg("--version").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if version.is_empty() {
        None
    } else {
        Some(version)
    }
}

/// 检测 lark-cli 是否安装，返回版本号
pub fn check_lark_cli() -> Option<String> {
    if !lark::lark_cli_exists() {
        return None;
    }
    lark::lark_cli_version().ok()
}

/// 检测飞书应用配置状态
///
/// 返回 (app_id, brand)，如果未配置返回 None
pub fn check_app_config() -> Option<(String, String)> {
    lark::config_show().ok().flatten()
}

/// 检测用户登录状态
///
/// 返回 (identity, token_status, user_name)
/// - identity: "user" / "bot" / "" (未登录)
/// - token_status: "ready" / "needs_refresh" / "" (未登录)
/// - user_name: 用户名（仅 user 身份有值）
pub fn check_login() -> (String, String, Option<String>) {
    match lark::whoami() {
        Ok((identity, token_status, user_name)) => {
            // identity 为空或 token_status 为空表示未登录
            if identity.is_empty() && token_status.is_empty() {
                (String::new(), String::new(), None)
            } else {
                (identity, token_status, user_name)
            }
        }
        Err(_) => (String::new(), String::new(), None),
    }
}

/// 完整环境检测：一步到位检测所有依赖
///
/// 调用此函数后即可判断应用是否可以正常工作
pub fn check_env() -> EnvStatus {
    let node_version = check_node();
    let lark_cli_version = check_lark_cli();

    let mut status = EnvStatus {
        node_installed: node_version.is_some(),
        node_version,
        lark_cli_installed: lark_cli_version.is_some(),
        lark_cli_version,
        ..Default::default()
    };
    status.lark_cli_compatible = status
        .lark_cli_version
        .as_deref()
        .is_some_and(|version| version.contains(SUPPORTED_LARK_CLI_VERSION));
    if status.lark_cli_installed && !status.lark_cli_compatible {
        status.check_errors.push(EnvCheckError {
            component: "lark_cli_version".to_string(),
            message: format!(
                "当前版本不在已验证范围内，建议使用 lark-cli {}",
                SUPPORTED_LARK_CLI_VERSION
            ),
        });
    }

    // lark-cli 未安装，后面的检测无法进行
    if !status.lark_cli_installed {
        return status;
    }

    // 配置与登录互不依赖，并行执行可减少环境页等待时间。
    let (config_result, login_result) = std::thread::scope(|scope| {
        let config = scope.spawn(lark::config_show);
        let login = scope.spawn(lark::whoami);
        (
            config
                .join()
                .unwrap_or_else(|_| Err(AppError::Other("配置检测线程异常".to_string()))),
            login
                .join()
                .unwrap_or_else(|_| Err(AppError::Other("登录检测线程异常".to_string()))),
        )
    });

    match config_result {
        Ok(Some((app_id, _brand))) => {
            status.app_configured = true;
            status.app_id = Some(app_id);
        }
        Ok(None) => {}
        Err(error) => status.check_errors.push(EnvCheckError {
            component: "app_config".to_string(),
            message: error.to_string(),
        }),
    }

    // 检测用户登录状态
    match login_result {
        Ok((identity, token_status, user_name)) if !identity.is_empty() => {
            status.logged_in =
                identity == "user" && (token_status == "ready" || token_status == "needs_refresh");
            status.token_status = Some(token_status);
            status.user_name = user_name;
        }
        Ok(_) => {}
        Err(error) => status.check_errors.push(EnvCheckError {
            component: "login".to_string(),
            message: error.to_string(),
        }),
    }

    status
}

// ============================================================================
// 安装与引导
// ============================================================================

/// 自动安装 lark-cli（npm install -g @larksuite/cli@latest）
///
/// 需要 Node.js 已安装
pub fn install_lark_cli() -> AppResult<String> {
    // 先检查 Node.js
    if check_node().is_none() {
        return Err(AppError::NodeNotFound);
    }

    // 执行 npm install -g @larksuite/cli@latest
    let output = Command::new("npm")
        .args([
            "install",
            "-g",
            &format!("@larksuite/cli@{}", SUPPORTED_LARK_CLI_VERSION),
        ])
        .output()
        .map_err(|e| AppError::Other(format!("npm 执行失败: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Other(format!(
            "npm install 失败: {}",
            stderr.trim()
        )));
    }

    // 验证安装是否成功
    match check_lark_cli() {
        Some(version) => Ok(version),
        None => Err(AppError::Other(
            "npm install 完成，但 lark-cli 仍未找到，可能需要重启终端或检查 PATH".to_string(),
        )),
    }
}

/// 初始化飞书应用（阻塞模式）
///
/// 执行 `lark-cli config init --new --brand feishu --lang zh`
/// 此命令会阻塞，直到用户在浏览器中完成应用创建。
/// 返回命令的完整输出（包含可能的 URL 信息）
pub fn init_app_config(brand: &str, lang: &str) -> AppResult<String> {
    lark::config_init(brand, lang)
}

/// 发起非阻塞飞书登录
///
/// 执行 `lark-cli auth login --domain docs --domain drive --domain wiki --no-wait --json`
/// 返回 device_code 和 verification_url，应用需要打开浏览器让用户完成授权
pub fn start_login(domains: &[&str]) -> AppResult<DeviceInfo> {
    let stdout = lark::auth_login_no_wait(domains)?;
    let json_str = extract_json(&stdout);

    // 解析非阻塞登录返回的 JSON
    // 格式可能是: { "device_code": "xxx", "verification_url": "https://..." }
    // 或者 lark-cli 自定义格式，需要解析
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| AppError::JsonParse(e.to_string()))?;

    // 尝试多种字段名
    let device_code = parsed
        .get("device_code")
        .or_else(|| parsed.get("deviceCode"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            let preview: String = stdout.chars().take(200).collect();
            AppError::JsonParse(format!(
                "无法从 auth login --no-wait 输出中提取 device_code: {}",
                preview
            ))
        })?
        .to_string();

    let verification_url = parsed
        .get("verification_url")
        .or_else(|| parsed.get("verificationUrl"))
        .or_else(|| parsed.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("https://accounts.feishu.cn/oauth/v1/device/verify")
        .to_string();

    Ok(DeviceInfo {
        device_code,
        verification_url,
    })
}

/// 用 device_code 完成登录（阻塞模式）
///
/// 执行 `lark-cli auth login --device-code <code>`
/// 阻塞直到用户完成授权或超时
pub fn complete_login(device_code: &str) -> AppResult<LoginResult> {
    match lark::auth_login_with_device_code(device_code) {
        Ok(_stdout) => {
            // 验证登录是否成功
            let (identity, token_status, user_name) = lark::whoami()?;
            let success =
                identity == "user" && (token_status == "ready" || token_status == "needs_refresh");

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
        Err(e) => Ok(LoginResult {
            success: false,
            user_name: None,
            error: Some(e.to_string()),
        }),
    }
}

/// 从可能包含日志行的输出中提取 JSON
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
