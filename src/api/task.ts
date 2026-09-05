// ============================================================================
// src/api/task.ts —— 下载任务相关 IPC
//
// 对应后端命令：
//   start_extract_wiki(wikiUrl, outputDir?, selectedTokens?) -> task_id
//   get_progress(taskId)       -> Progress
//   cancel_task(taskId)        -> ()
//   get_task_result(taskId)    -> WikiTaskResult
//   dismiss_task_result(taskId)-> ()
//   list_task_history()        -> WikiTaskResult[]
//
// 全部是 Tauri command 的薄包装。进度轮询逻辑见 ../composables/useTaskProgress.ts
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { ScanMode } from "./wiki";
import type { Progress, WikiTaskResult } from "./types";

/**
 * 异步启动批量下载任务，返回 taskId。
 * 后端在 spawn 的 task 里跑扫描+导出，进度通过 get_progress 轮询。
 *
 * @param wikiUrl  知识库根节点链接
 * @param outputDir 输出目录；undefined 表示用 Settings.output_dir
 * @param selectedTokens 选中节点的 token；undefined 表示全选
 * @param scanMode 扫描模式；undefined 表示 auto（与扫描时保持一致）
 */
export async function startExtractWiki(
  wikiUrl: string,
  outputDir?: string,
  selectedTokens?: string[],
  scanMode?: ScanMode
): Promise<string> {
  return invoke<string>("start_extract_wiki", {
    wikiUrl,
    outputDir: outputDir ?? null,
    selectedTokens: selectedTokens ?? null,
    scanMode: scanMode ?? null,
  });
}

/** 拉一次任务进度。后端会刷新 timing 字段。 */
export async function getProgress(taskId: string): Promise<Progress> {
  return invoke<Progress>("get_progress", { taskId });
}

/** 请求取消任务（任务自行检查 cancelled 标志）。 */
export async function cancelTask(taskId: string): Promise<void> {
  return invoke<void>("cancel_task", { taskId });
}

/** 拉已完成任务的完整结果（含 items 与可能的 error）。 */
export async function getTaskResult(taskId: string): Promise<WikiTaskResult> {
  return invoke<WikiTaskResult>("get_task_result", { taskId });
}

/** 从已完成列表移除一条。 */
export async function dismissTaskResult(taskId: string): Promise<void> {
  return invoke<void>("dismiss_task_result", { taskId });
}

/** 拉所有已完成任务历史（最近 24 小时，最多 100 条）。 */
export async function listTaskHistory(): Promise<WikiTaskResult[]> {
  return invoke<WikiTaskResult[]>("list_task_history");
}