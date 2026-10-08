// ============================================================================
// src/api/env.ts —— 环境体检 / 初始化 / 飞书登录相关 IPC
//
// 对应后端命令：
//   check_env()                  -> EnvStatus
//   setup_lark_cli()             -> string（消息）
//   start_app_init()             -> AppInitStatus（后台流式启动创建向导）
//   get_app_init_status()        -> AppInitStatus（轮询：抓到 url 后自动打开浏览器）
//   start_login()                -> DeviceInfo（非阻塞，返回设备码 + 授权链接）
//   complete_login(deviceCode)   -> LoginResult（单次阻塞等待授权，勿并发轮询）
//   qr_svg(text)                 -> string（把授权链接渲染成二维码 SVG，纯本地）
//   logout()                     -> string（消息，清除 lark-cli token）
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type {
  AppInitStatus,
  DeviceInfo,
  EnvStatus,
  LoginResult,
  ScopeCheck,
} from "./types";

export async function checkEnv(): Promise<EnvStatus> {
  return invoke<EnvStatus>("check_env");
}

/** 单独巡检授权范围（补完权限点后立刻复查用；体检里的 scope_check 是同一实现的附带结果） */
export async function checkScopes(): Promise<ScopeCheck> {
  return invoke<ScopeCheck>("check_scopes");
}

export async function setupLarkCli(): Promise<string> {
  return invoke<string>("setup_lark_cli");
}

/** 后台启动飞书应用创建向导（阻塞式浏览器向导，命令立即返回、后台运行） */
export async function startAppInit(
  brand = "feishu",
  lang = "zh",
): Promise<AppInitStatus> {
  return invoke<AppInitStatus>("start_app_init", { brand, lang });
}

/** 查询创建向导实时状态：轮询到 url 后自动打开浏览器，running=false 即结束 */
export async function getAppInitStatus(): Promise<AppInitStatus> {
  return invoke<AppInitStatus>("get_app_init_status");
}

export async function startLogin(): Promise<DeviceInfo> {
  return invoke<DeviceInfo>("start_login");
}

export async function completeLogin(deviceCode: string): Promise<LoginResult> {
  return invoke<LoginResult>("complete_login", { deviceCode });
}

/**
 * 把授权链接渲染成二维码 SVG（后端纯本地生成，不联网、不写临时文件）。
 *
 * 登录面板用二维码 + 链接两条路并给：本机浏览器点「开通并授权」无反应的环境下，
 * 手机扫码（飞书 / 豆包 / 任意浏览器，账号同源）照样能完成授权。
 */
export async function qrSvg(text: string): Promise<string> {
  return invoke<string>("qr_svg", { text });
}

/**
 * 用**隔离浏览器**打开链接（独立 profile 的干净实例，创建向导页与授权页共用）。
 *
 * 为什么不用系统默认浏览器：授权页要求浏览器里已有豆包/飞书登录态，且对浏览器环境
 * 敏感——日常浏览器常见"点开通并授权毫无反应"，而独立 profile 的干净实例实测一次成功。
 * 本调用只负责打开并带上链接，扫码登录与点「开通并授权」仍由用户完成。
 */
export async function openIsolatedBrowser(url: string): Promise<string> {
  return invoke<string>("open_isolated_browser", { url });
}

export async function logout(): Promise<string> {
  return invoke<string>("logout");
}