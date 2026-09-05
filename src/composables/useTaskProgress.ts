// ============================================================================
// src/composables/useTaskProgress.ts —— 任务进度轮询
//
// 给定 taskId 响应式引用，启动 setInterval 拉 get_progress；
// 把进度变化写入 useTaskStore 的相应字段（phase / done / currentDoc /
// estimatedRemainingSeconds / nodeStates / successCount / failedCount）；
// 任务进入终态（completed / failed / cancelled）时拉一次 get_task_result
// 把 items 写入 store，停轮询。
//
// 用法（store.start() 内）：
//   const stop = useTaskProgress(toRef(store, 'taskId'));
//   stopProgress = stop;  // store.cancel / reset 时调它
//
// 设计要点：
//   - 800ms 间隔（人眼可感的「流畅」+ 后端无压力）
//   - 失败重试 3 次再放弃
//   - taskId 变化自动重启；传 null 立即停
// ============================================================================

import { type Ref, watch } from "vue";
import { useTaskStore } from "../stores/task";
import { getProgress, getTaskResult } from "../api/task";

const POLL_INTERVAL_MS = 800;
const MAX_CONSECUTIVE_ERRORS = 3;

export function useTaskProgress(taskId: Ref<string | null>): () => void {
  const store = useTaskStore();
  let timer: number | null = null;
  let consecutiveError = 0;

  function clearTimer() {
    if (timer != null) {
      window.clearInterval(timer);
      timer = null;
    }
  }

  async function tick() {
    const id = taskId.value;
    if (!id) return;

    let progress;
    try {
      progress = await getProgress(id);
      consecutiveError = 0;
    } catch (err) {
      consecutiveError += 1;
      if (consecutiveError >= MAX_CONSECUTIVE_ERRORS) {
        clearTimer();
        console.warn("[useTaskProgress] progress poll failed 3x", err);
      }
      return;
    }

    store.applyProgress(progress);

    if (
      progress.status === "completed" ||
      progress.status === "failed" ||
      progress.status === "cancelled"
    ) {
      clearTimer();
      try {
        const taskResult = await getTaskResult(id);
        store.applyTaskResult(taskResult);
      } catch (err) {
        console.warn("[useTaskProgress] getTaskResult failed", err);
      }
    }
  }

  function start() {
    clearTimer();
    void tick();
    timer = window.setInterval(tick, POLL_INTERVAL_MS);
  }

  watch(
    taskId,
    (id) => {
      if (id) start();
      else clearTimer();
    },
    { immediate: true }
  );

  return () => {
    clearTimer();
  };
}