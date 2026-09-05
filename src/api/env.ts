// ============================================================================
// src/api/env.ts —— 环境体检 / 初始化 / 飞书登录相关 IPC
//
// 对应后端命令：
//   check_env()                  -> EnvStatus
//   setup_lark_cli()             -> string（消息）
//   init_app(brand, lang)        -> string（消息）
//   start_login()                -> DeviceInfo（非阻塞，返回后前端打开浏览器）
//   complete_login(deviceCode)   -> LoginResult（单次阻塞等待授权，勿并发轮询）
//   login_feishu_blocking()      -> LoginResult（一步到位阻塞登录，备用入口）
//   logout()                     -> string（消息，清除 lark-cli token）
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { DeviceInfo, EnvStatus, LoginResult } from "./types";

export async function checkEnv(): Promise<EnvStatus> {
  return invoke<EnvStatus>("check_env");
}

export async function setupLarkCli(): Promise<string> {
  return invoke<string>("setup_lark_cli");
}

export async function initApp(brand: string, lang: string): Promise<string> {
  return invoke<string>("init_app", { brand, lang });
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