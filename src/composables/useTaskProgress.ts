// ============================================================================
// src/composables/useTaskProgress.ts —— 任务进度轮询
//
// 给定 taskId 响应式引用，启动 setInterval 拉 get_progress；
// 把进度变化写入 useTaskStore 的相应字段（phase / done / currentDoc /
// estimatedRemainingSeconds / nodeStates / successCount / failedCount）；
// 任务进入终态（completed / failed / cancelled）时拉一次 get_task_result
// 把 items 写入 store，停轮询。
//
// 用法（store 初始化时注册一次，watch taskId 自管理启停）：
//   useTaskProgress(taskIdRef);
//
// 设计要点：
//   - 800ms 间隔（人眼可感的「流畅」+ 后端无压力）
//   - 失败重试 3 次再放弃
//   - taskId 变化自动重启；传 null 立即停
//   - 每个 await 之后都校验 taskId 未变：reset / 清空 / 换新任务后
//     丢弃迟到的旧响应，避免旧任务终态覆盖新任务状态
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
  /** 防止某次请求慢于间隔导致两个 tick 同时跑 */
  let inFlight = false;

  function clearTimer() {
    if (timer != null) {
      window.clearInterval(timer);
      timer = null;
    }
  }

  async function tick() {
    const id = taskId.value;
    if (!id || inFlight) return;
    inFlight = true;
    try {
      let progress;
      try {
        progress = await getProgress(id);
        consecutiveError = 0;
      } catch (err) {
        consecutiveError += 1;
        if (consecutiveError >= MAX_CONSECUTIVE_ERRORS) {
          clearTimer();
          if (taskId.value === id) {
            // 进度轮询彻底失败时兜底结束，避免永远卡在「运行中」
            store.applyTaskResultError(String(err));
          }
          console.warn("[useTaskProgress] progress poll failed 3x", err);
        }
        return;
      }

      // 请求期间任务已被 reset/清空/更换：丢弃迟到响应
      if (taskId.value !== id) return;

      store.applyProgress(progress);

      if (
        progress.status === "completed" ||
        progress.status === "failed" ||
        progress.status === "cancelled"
      ) {
        clearTimer();
        // 结果表入库与任务表摘除之间有一个极短窗口：get_progress 恰在此
        // 时读到终态、随后 get_task_result 会报"尚未完成"。这里带间隔短
        // 重试几次再放弃，避免一次微妙竞态就让界面以"明细缺失"收尾。
        for (let attempt = 0; attempt < 5; attempt++) {
          if (taskId.value !== id) return; // 任务已被 reset/清空/更换
          try {
            const taskResult = await getTaskResult(id);
            if (taskId.value === id) store.applyTaskResult(taskResult);
            return;
          } catch (err) {
            const isLast = attempt === 4;
            if (isLast || taskId.value !== id) {
              if (taskId.value === id) store.applyTaskResultError(String(err));
              console.warn("[useTaskProgress] getTaskResult failed", err);
              return;
            }
            await sleep(500);
          }
        }
      }
    } finally {
      inFlight = false;
    }
  }

  function start() {
    clearTimer();
    // 上一个任务遗留的失败计数 / 在途标记不能带进新任务
    consecutiveError = 0;
    inFlight = false;
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

/** 供结果重试间隔使用 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}