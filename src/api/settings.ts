// ============================================================================
// src/api/settings.ts —— 设置 / 输出目录相关 IPC
//
// 对应后端命令：
//   set_settings(settings)          -> ()
//   get_settings_status()           -> SettingsStatus
//   preflight_output_dir(path)      -> OutputPreflight
//   open_output_dir(path)           -> ()
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { OutputPreflight, Settings, SettingsStatus } from "./types";

export async function setSettings(settings: Settings): Promise<void> {
  return invoke<void>("set_settings", { settings });
}

export async function getSettingsStatus(): Promise<SettingsStatus> {
  return invoke<SettingsStatus>("get_settings_status");
}

export async function preflightOutputDir(path: string): Promise<OutputPreflight> {
  return invoke<OutputPreflight>("preflight_output_dir", { path });
}

export async function openOutputDir(path: string): Promise<void> {
  return invoke<void>("open_output_dir", { path });
}