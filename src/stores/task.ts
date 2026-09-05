// ============================================================================
// src/stores/task.ts —— 当前下载任务状态（结构占位）
//
// 职责（M2 装 pinia 后填充，defineStore('task', ...)）：
//   state : taskId / progress(Progress) / phase / error / result(WikiTaskResult)
//   actions: start(...)    api/task.ts start_extract_wiki，记录 taskId
//            cancel()      api/task.ts cancel_task
//            clear()       api/task.ts dismiss_task_result
//   getters: 是否运行中 / 失败清单 / 总进度比（配合 useTaskProgress 轮询）
//
// 说明：当前不 import pinia（依赖未装，避免破基线）。
// ============================================================================

export {};
