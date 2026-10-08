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
//! ## 浏览器优先级：Chrome 优先，缺失才退 Edge（2026-10-09 用户明确要求）
//!
//! 候选顺序为 Chrome（`Program Files` / `Program Files (x86)` / `LocalAppData`）→ Edge。
//! 原实现是 Edge 优先，用户实测后要求"弹出来的应当是 Chrome 的干净实例"：
//! 日常浏览器就是 Chrome，弹出的窗口与预期一致才不会被误当成"打开了我的浏览器"。
//! 这条优先级由 `chrome_comes_first_and_edge_is_fallback` 单测锁死，改动会直接红。
//!
//! ## 失败面收敛：三种结果都算"已打开"，不让用户卡在登录页
//!
//! 1. 新实例存活 → 干净窗口已出现；
//! 2. 子进程秒退 → Chromium 同 profile 单实例机制把请求**交接给已在运行的隔离实例**
//!    （页面已在那个窗口打开，属正常现象，不是失败）；
//! 3. 全部候选都缺失或启动失败 → 退回系统默认浏览器，并写 WARN 日志留线索。
//!
//! 每一步（选了哪个浏览器、pid、是否交接、profile 路径）都写进应用日志，
//! 事后排查不需要再猜。
//!
//! ## profile 常驻是刻意的
//!
//! `{config_dir}/LarkReader/auth-browser-profile/<chrome|edge>` 会被复用：首次扫码登录后，
//! 后续授权直接进入授权页，不必反复扫码。按浏览器分一级子目录，避免两个 Chromium 内核
//! 共用同一份 profile 互相污染。目录与设置/日志同级，方便整目录清理。

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use crate::error::{AppError, AppResult};

/// 启动后等多久，用于判断"新实例起来了"还是"请求被交接给已有实例"。
///
/// Chromium 家族对同一 profile 是单实例：已有实例在跑时，新进程会立刻退出。
/// 1.2s 足够区分（实测秒退 < 100ms，正常起窗 > 1s）。
const HANDOFF_PROBE: Duration = Duration::from_millis(1200);

/// 隔离 profile 目录（`{config_dir}/LarkReader/auth-browser-profile/<浏览器>`）
pub fn profile_dir(browser: &str) -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("LarkReader")
        .join("auth-browser-profile")
        .join(browser)
}

/// 浏览器标识 → 给用户看的展示名
fn display_name(browser: &str) -> &'static str {
    match browser {
        "chrome" => "Chrome",
        _ => "Edge",
    }
}

/// 候选浏览器（按优先级）：**Chrome 优先**，缺失时退到 Edge。
///
/// 返回 `(浏览器标识, 可执行文件路径)`；标识用于分隔各自的 profile 目录
/// （Chrome 与 Edge 同为 Chromium 内核，但不应共用同一份 profile）。
fn candidates() -> Vec<(&'static str, PathBuf)> {
    candidates_for(
        &std::env::var("ProgramFiles").unwrap_or_default(),
        &std::env::var("ProgramFiles(x86)").unwrap_or_default(),
        &std::env::var("LOCALAPPDATA").unwrap_or_default(),
    )
}

/// `candidates` 的纯函数形态（安装根目录作为入参），便于单测锁住优先级与空目录处理。
fn candidates_for(pf: &str, pf86: &str, local: &str) -> Vec<(&'static str, PathBuf)> {
    let mut out: Vec<(&'static str, PathBuf)> = Vec::new();
    let mut add = |key: &'static str, base: &str, rel: &str| {
        // 安装根目录为空时直接跳过，避免拼出 `\Microsoft\Edge\...` 这种假路径
        if !base.trim().is_empty() {
            out.push((key, PathBuf::from(format!("{base}{rel}"))));
        }
    };

    // 首选 Chrome：用户预期与日常浏览器一致（2026-10-09 明确要求，勿改回 Edge 优先）
    add("chrome", pf, r"\Google\Chrome\Application\chrome.exe");
    add("chrome", pf86, r"\Google\Chrome\Application\chrome.exe");
    add("chrome", local, r"\Google\Chrome\Application\chrome.exe");
    // 兜底 Edge：Windows 系统自带，最稳
    add("edge", pf86, r"\Microsoft\Edge\Application\msedge.exe");
    add("edge", pf, r"\Microsoft\Edge\Application\msedge.exe");
    add("edge", local, r"\Microsoft\Edge\Application\msedge.exe");
    out
}

/// 用隔离 profile 打开链接；找不到可用浏览器时退回系统默认打开方式。
///
/// 返回一句可直接给用户看的中文说明（用哪个浏览器、是否复用了已有窗口）。
/// 失败面收敛策略见模块头注释第 3 节。
pub fn open_isolated(url: &str) -> AppResult<String> {
    let mut failures: Vec<String> = Vec::new();

    for (browser, exe) in candidates() {
        if !exe.is_file() {
            continue;
        }
        // profile 按浏览器分开：Chrome 与 Edge 各用各的目录，避免互相污染
        let profile = profile_dir(browser);
        std::fs::create_dir_all(&profile).map_err(AppError::Io)?;

        match Command::new(&exe)
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--new-window")
            .arg(url)
            .spawn()
        {
            Ok(mut child) => {
                let pid = child.id();
                // 存活 = 自己起了实例；秒退 = 交接给已在运行的隔离实例（两种都算成功）
                std::thread::sleep(HANDOFF_PROBE);
                let handoff = matches!(child.try_wait(), Ok(Some(_)));
                let label = display_name(browser);
                crate::logger::info(format!(
                    "授权窗口已打开：{label} pid={pid} handoff={handoff} exe={} profile={}",
                    exe.display(),
                    profile.display()
                ));
                return Ok(if handoff {
                    format!("已用 {label} 的干净窗口打开授权页（复用已打开的那个独立窗口）")
                } else {
                    format!("已用 {label} 的干净窗口打开授权页（独立窗口，没有你的书签与扩展）")
                });
            }
            Err(error) => failures.push(format!("{} 启动失败：{error}", exe.display())),
        }
    }

    if !failures.is_empty() {
        crate::logger::warn(format!("隔离浏览器启动失败：{}", failures.join("；")));
    }
    crate::logger::warn("未找到可用的隔离浏览器，退回系统默认浏览器打开授权页");

    // 兜底：与旧行为一致（系统默认浏览器）。此时没有隔离环境，
    // 用户仍可用面板上的链接/二维码自行处理。
    tauri_plugin_opener::open_url(url, None::<&str>)
        .map_err(|e| AppError::Other(format!("打开浏览器失败: {e}")))?;
    Ok("已用系统默认浏览器打开授权页（未启用隔离配置；若点「开通并授权」没反应，请改用面板里的二维码）"
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 锁死浏览器优先级：改回 Edge 优先会让这条测试直接红。
    #[test]
    fn chrome_comes_first_and_edge_is_fallback() {
        let list = candidates_for(r"C:\PF", r"C:\PF86", r"C:\Local");
        assert_eq!(list.len(), 6, "Chrome / Edge 各 3 个安装位置");
        assert!(
            list[..3].iter().all(|(key, _)| *key == "chrome"),
            "前三个候选必须是 Chrome（2026-10-09 用户要求，勿回退）"
        );
        assert!(
            list[3..].iter().all(|(key, _)| *key == "edge"),
            "Chrome 之后才是 Edge 兜底"
        );
        assert!(list[0]
            .1
            .to_string_lossy()
            .ends_with(r"C:\PF\Google\Chrome\Application\chrome.exe"));
    }

    /// 根目录缺失时不能拼出 `\Microsoft\Edge\...` 这类假路径。
    #[test]
    fn empty_install_roots_are_skipped() {
        let list = candidates_for(r"C:\PF", "", r"C:\Local");
        assert_eq!(list.len(), 4, "PF86 为空时只应少一个候选");
        assert!(
            list.iter()
                .all(|(_, path)| !path.to_string_lossy().starts_with('\\')),
            "任何候选都必须是带盘符的绝对路径"
        );
    }

    /// 两个内核不能共用 profile，否则互相污染。
    #[test]
    fn profiles_are_separated_per_browser() {
        let chrome = profile_dir("chrome");
        let edge = profile_dir("edge");
        assert_ne!(chrome, edge);
        assert!(chrome.ends_with("chrome"));
        assert!(edge.ends_with("edge"));
    }
}
