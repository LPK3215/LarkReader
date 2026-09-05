// ============================================================================
// src/stores/history.ts —— 任务历史列表
//
// 数据：api/task.ts list_task_history -> WikiTaskResult[]（按完成时间倒序）
// 操作：打开产物目录（openOutputDir）、删除单条（dismissTaskResult）
//
// 后端约定：历史持久化到本地，保留最近 24 小时且最多 100 条，按完成时间淘汰。
// 真机专享：所有动作走 IPC；不再保留浏览器假数据兜底。
// ============================================================================

import { ref } from "vue";
import { defineStore } from "pinia";
import type { WikiTaskResult } from "../api/types";
import { dismissTaskResult, listTaskHistory } from "../api/task";
import { openOutputDir } from "../api/settings";

export const useHistoryStore = defineStore("history", () => {
  const records = ref<WikiTaskResult[]>([]);
  const loading = ref(false);
  const lastError = ref<string | null>(null);

  /** 进入页面时拉一次；后续删除/打开目录成功后调用 refresh()。 */
  async function load() {
    loading.value = true;
    lastError.value = null;
    try {
      records.value = await listTaskHistory();
    } catch (err) {
      lastError.value = `加载历史失败：${String(err)}`;
      records.value = [];
    } finally {
      loading.value = false;
    }
  }

  /** 重新拉一次（删除或操作后调用）。 */
  async function refresh() {
    return load();
  }

  /** 删除单条本地记录。 */
  async function remove(taskId: string) {
    try {
      await dismissTaskResult(taskId);
      records.value = records.value.filter((r) => r.task_id !== taskId);
    } catch (err) {
      lastError.value = `删除失败：${String(err)}`;
    }
  }

  /** 在系统文件管理器里打开该任务的产物目录。 */
  async function openDir(path: string) {
    try {
      await openOutputDir(path);
    } catch (err) {
      lastError.value = `打开目录失败：${String(err)}`;
    }
  }

  return {
    records,
    loading,
    lastError,
    load,
    refresh,
    remove,
    openDir,
  };
});