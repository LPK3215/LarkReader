// ============================================================================
// src/api/env.ts —— 环境体检 / 初始化 / 飞书登录相关 IPC
//
// 对应后端命令：
//   check_env()                  -> EnvStatus
//   setup_lark_cli()             -> string（消息）
//   init_app(brand, lang)        -> string（消息，同步阻塞版，勿 UI 直调）
//   start_app_init()             -> AppInitStatus（后台流式启动创建向导）
//   get_app_init_status()        -> AppInitStatus（轮询：抓到 url 后自动打开浏览器）
//   start_login()                -> DeviceInfo（非阻塞，返回后前端打开浏览器）
//   complete_login(deviceCode)   -> LoginResult（单次阻塞等待授权，勿并发轮询）
//   login_feishu_blocking()      -> LoginResult（一步到位阻塞登录，备用入口）
//   logout()                     -> string（消息，清除 lark-cli token）
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { AppInitStatus, DeviceInfo, EnvStatus, LoginResult } from "./types";

export async function checkEnv(): Promise<EnvStatus> {
  return invoke<EnvStatus>("check_env");
}

export async function setupLarkCli(): Promise<string> {
  return invoke<string>("setup_lark_cli");
}

export async function initApp(brand: string, lang: string): Promise<string> {
  return invoke<string>("init_app", { brand, lang });
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

export async function loginFeishuBlocking(): Promise<LoginResult> {
  return invoke<LoginResult>("login_feishu_blocking");
}

export async function logout(): Promise<string> {
  return invoke<string>("logout");
}