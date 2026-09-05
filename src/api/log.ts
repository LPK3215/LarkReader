// ============================================================================
// src/api/log.ts —— 运行日志 IPC
//
// 对应后端命令：
//   list_log_files()          -> LogFileMeta[]
//   read_log_file(name)       -> LogFileContent
//   open_log_dir()            -> ()
//
// 运行日志由后端 logger.rs 按天写到 {config_dir}/LarkReader/logs/，
// 「运行日志」页（/logs）负责把文件渲染出来。
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { LogFileContent, LogFileMeta } from "./types";

/** 列出日志目录里的所有日志文件（最新在前）。 */
export async function listLogFiles(): Promise<LogFileMeta[]> {
  return invoke<LogFileMeta[]>("list_log_files");
}

/** 读取指定日志文件的文本内容（过大时后端只返回末尾部分）。 */
export async function readLogFile(name: string): Promise<LogFileContent> {
  return invoke<LogFileContent>("read_log_file", { name });
}

/** 打开日志目录（在系统文件管理器中查看）。 */
export async function openLogDir(): Promise<void> {
  return invoke<void>("open_log_dir");
}
