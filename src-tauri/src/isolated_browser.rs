//! 隔离浏览器：用**独立 profile** 拉起系统浏览器，打开创建向导 / 设备码授权页。
//!
//! ## 为什么需要它（2026-10-09 本机实测，勿删）
//!
//! 设备码授权页要求浏览器里**已有豆包 / 飞书登录态**，而且对浏览器环境很敏感：
//!
//! | 环境 | 现象 |
//! |---|---|
//! | 用户日常浏览器（有登录态、有扩展、走代理） | 页面能打开、权限清单也正常，但点「开通并授权」**毫无反应**（无报错、无提示） |
//! | 手机任意浏览器（无登录态） | 直接提示"链接失效"（拿不到登录态） |
//! | **独立 profile 的干净真实例** | 先跳"扫码登录"（手机豆包/飞书扫码）→ 登录后同一流程继续 → 点授权**一次成功** |
//!
//! 所以创建应用向导与登录授权两处都改成用它：**自动打开 + 自动带上链接**，
//! 但绝不代替用户做后续操作（扫码登录、点「开通并授权」由用户自己完成）。
//!
//! ## 为什么用外部浏览器而不是应用内 WebView
//!
//! 应用内 WebView（隔离 data_directory）理论上等价，但授权页依赖的第三方登录
//! 组件在 WebView2 上的表现未经实测；外部浏览器是**已被验证可行**的那条路，
//! 且零新增依赖。
//!
//! ## profile 常驻是刻意的
//!
//! `{config_dir}/LarkReader/auth-browser-profile` 会被复用：首次扫码登录后，
//! 后续授权直接进入授权页，不必反复扫码。目录与设置/日志同级，方便整目录清理。

use std::path::PathBuf;
use std::process::Command;

use crate::error::{AppError, AppResult};

/// 隔离 profile 目录（`{config_dir}/LarkReader/auth-browser-profile`）
pub fn profile_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("LarkReader")
        .join("auth-browser-profile")
}

/// 候选浏览器可执行文件（按优先级；Windows 上 Edge 系统自带，最稳）
fn candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    let pf = std::env::var("ProgramFiles").unwrap_or_default();
    let pf86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();

    let mut add = |path: String| {
        if !path.trim().is_empty() {
            out.push(PathBuf::from(path));
        }
    };
    add(format!(r"{pf86}\Microsoft\Edge\Application\msedge.exe"));
    add(format!(r"{pf}\Microsoft\Edge\Application\msedge.exe"));
    add(format!(r"{local}\Microsoft\Edge\Application\msedge.exe"));
    add(format!(r"{pf}\Google\Chrome\Application\chrome.exe"));
    add(format!(r"{pf86}\Google\Chrome\Application\chrome.exe"));
    add(format!(r"{local}\Google\Chrome\Application\chrome.exe"));
    out
}

/// 用隔离 profile 打开链接；找不到可用浏览器时退回系统默认打开方式。
///
/// 返回一句可直接给用户看的中文说明（用哪个浏览器、profile 在哪）。
pub fn open_isolated(url: &str) -> AppResult<String> {
    let profile = profile_dir();
    std::fs::create_dir_all(&profile).map_err(AppError::Io)?;

    for exe in candidates() {
        if !exe.is_file() {
            continue;
        }
        let spawned = Command::new(&exe)
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--new-window")
            .arg(url)
            .spawn();
        if spawned.is_ok() {
            return Ok(format!(
                "已用隔离浏览器打开（{}，配置目录 {}）",
                exe.display(),
                profile.display()
            ));
        }
    }

    // 兜底：与旧行为一致（系统默认浏览器）。此时没有隔离环境，
    // 用户仍可用面板上的链接/二维码自行处理。
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|e| AppError::Other(format!("打开浏览器失败: {e}")))?;
    Ok("已用系统默认浏览器打开（未启用隔离配置，遇到点不动请改用面板里的二维码）".to_string())
}
