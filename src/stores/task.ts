// ============================================================================
// src/stores/task.ts —— 当前下载任务状态
//
// state : stage(工作台阶段) / taskId / phase / 计数 / 节点状态 / items / result
// actions:
//   scan(url)      get_wiki_tree
//   start()        start_extract_wiki + 注册 useTaskProgress 轮询
//   cancel()       cancel_task + 停轮询
//   reset()        回到 tree 状态，停轮询
//   clearAll()     回到 empty 状态
//   applyProgress(p)   useTaskProgress 回调；只写变化的字段
//   applyTaskResult(r) useTaskProgress 终态回调；填 items / outputRoot
//
// 真机专享：所有动作走 IPC；不再保留浏览器假数据兜底。
// ============================================================================

import { computed, ref } from "vue";
import { defineStore } from "pinia";
import type {
  ExportItemResult,
  Progress,
  TaskPhase,
  WikiNode,
  WikiNodeType,
  WikiTaskResult,
} from "../api/types";
import { getWikiTree } from "../api/wiki";
import {
  cancelTask as apiCancelTask,
  startExtractWiki,
} from "../api/task";
import { useTaskProgress } from "../composables/useTaskProgress";

/** 工作台阶段：空态 -> 已扫树 -> 任务中 -> 已完成 */
export type WorkspaceStage = "empty" | "tree" | "running" | "done";

/** 进行中的阶段序列（finished 单独作为终点，不计入流程） */
export const PHASE_FLOW: TaskPhase[] = [
  "queued",
  "checking_output",
  "scanning_wiki",
  "exporting_document",
  "exporting_sheet",
  "exporting_bitable",
  "exporting_file",
  "finalizing",
];

export const PHASE_LABEL: Record<TaskPhase, string> = {
  queued: "排队中",
  checking_output: "检查输出目录",
  scanning_wiki: "扫描知识库",
  exporting_document: "导出文档",
  exporting_sheet: "导出表格",
  exporting_bitable: "导出多维表格",
  exporting_file: "下载附件",
  finalizing: "收尾整理",
  finished: "已完成",
};

/** 单个节点在任务中的结果状态 */
export type NodeRunState = "success" | "partial" | "failed" | "skipped" | "running";

export const useTaskStore = defineStore("task", () => {
  // ---- 状态 ----
  const stage = ref<WorkspaceStage>("empty");
  const taskId = ref<string | null>(null);
  const wikiUrl = ref("");
  const tree = ref<WikiNode | null>(null);
  const selectedTokens = ref<string[]>([]);
  const phase = ref<TaskPhase>("queued");
  const total = ref(0);
  const done = ref(0);
  const successCount = ref(0);
  const failedCount = ref(0);
  const currentDoc = ref<string | null>(null);
  const currentItemType = ref<WikiNodeType | null>(null);
  const estimatedRemainingSeconds = ref<number | null>(null);
  const cancelled = ref(false);
  const outputRoot = ref("");
  const items = ref<ExportItemResult[]>([]);
  const taskBarVisible = ref(true);
  const nodeStates = ref<Record<string, NodeRunState>>({});
  const lastError = ref<string | null>(null);

  // ---- 内部 ----
  let stopProgress: (() => void) | null = null;

  // ---- 派生 ----
  const running = computed(() => stage.value === "running");
  const finished = computed(() => stage.value === "done");
  const progressPercent = computed(() =>
    total.value > 0 ? Math.min(100, Math.round((done.value / total.value) * 100)) : 0
  );
  const phaseIndex = computed(() => PHASE_FLOW.indexOf(phase.value));
  const phaseLabel = computed(() => PHASE_LABEL[phase.value]);

  const selectedBreakdown = computed(() => {
    const counts: Record<WikiNodeType, number> = {
      doc: 0,
      sheet: 0,
      bitable: 0,
      file: 0,
      folder: 0,
      other: 0,
    };
    const root = tree.value;
    if (!root) return counts;
    const walk = (node: WikiNode) => {
      if (selectedTokens.value.includes(node.node_token)) {
        counts[node.obj_type] += 1;
      }
      node.children.forEach(walk);
    };
    walk(root);
    return counts;
  });

  const hasFailure = computed(() => failedCount.value > 0);

  // ---- 工具 ----
  function resetProgressFields() {
    phase.value = "queued";
    total.value = 0;
    done.value = 0;
    successCount.value = 0;
    failedCount.value = 0;
    currentDoc.value = null;
    currentItemType.value = null;
    estimatedRemainingSeconds.value = null;
    nodeStates.value = {};
    items.value = [];
    cancelled.value = false;
    lastError.value = null;
  }

  function stopPolling() {
    if (stopProgress) {
      stopProgress();
      stopProgress = null;
    }
  }

  // ---- 动作 ----

  /**
   * 扫描知识库结构。失败抛错由调用方 catch（UI 弹 n-message-error）。
   */
  async function scan(url: string) {
    lastError.value = null;
    wikiUrl.value = url;
    const node = await getWikiTree(url);
    tree.value = node;
    selectedTokens.value = collectDocTokens(node);
    stage.value = "tree";
    resetProgressFields();
  }

  /**
   * 启动下载。后端返回 taskId，启动 800ms 轮询；终态由 useTaskProgress 自动收尾。
   */
  async function start() {
    if (stage.value !== "tree") return;
    if (!tree.value || selectedTokens.value.length === 0) return;
    lastError.value = null;
    resetProgressFields();
    cancelled.value = false;
    taskBarVisible.value = true;
    stopPolling();
    try {
      const id = await startExtractWiki(wikiUrl.value, undefined, selectedTokens.value);
      taskId.value = id;
      total.value = selectedTokens.value.length;
      stage.value = "running";
      phase.value = "checking_output";
      // taskId 变化时 useTaskProgress 内部 watch 会处理
      stopProgress = useTaskProgress(taskId);
    } catch (err) {
      lastError.value = String(err);
      stage.value = "tree"; // 回到 tree，让用户重试
    }
  }

  /** 取消任务。 */
  async function cancel() {
    cancelled.value = true;
    stopPolling();
    if (taskId.value) {
      try {
        await apiCancelTask(taskId.value);
      } catch (err) {
        console.warn("[task.cancel] cancel_task failed", err);
      }
    }
  }

  /** 回到 tree 状态，可重新勾选再下。 */
  function reset() {
    stopPolling();
    taskId.value = null;
    stage.value = "tree";
    resetProgressFields();
  }

  /** 回到 empty 状态。 */
  function clearAll() {
    stopPolling();
    stage.value = "empty";
    tree.value = null;
    selectedTokens.value = [];
    wikiUrl.value = "";
    resetProgressFields();
  }

  // ---- useTaskProgress 回调 ----

  /**
   * 把 Progress 映射到 store。只写变化的字段，避免无意义渲染。
   */
  function applyProgress(p: Progress) {
    if (phase.value !== p.phase) phase.value = p.phase;
    if (total.value !== p.total) total.value = p.total;
    if (done.value !== p.done) done.value = p.done;
    if (successCount.value !== p.success_count) successCount.value = p.success_count;
    if (failedCount.value !== p.failed_count) failedCount.value = p.failed_count;
    if (currentDoc.value !== p.current_doc) currentDoc.value = p.current_doc;
    if (currentItemType.value !== p.current_item_type) currentItemType.value = p.current_item_type;
    if (estimatedRemainingSeconds.value !== p.estimated_remaining_seconds) {
      estimatedRemainingSeconds.value = p.estimated_remaining_seconds;
    }
    // 终态在 applyTaskResult 里处理
  }

  /**
   * 终态回调：把 WikiTaskResult 落到 store。
   */
  function applyTaskResult(r: WikiTaskResult) {
    stage.value = "done";
    phase.value = "finished";
    currentDoc.value = null;
    currentItemType.value = null;
    estimatedRemainingSeconds.value = 0;
    if (r.result) {
      outputRoot.value = r.result.output_root;
      items.value = r.result.items;
      total.value = r.result.items.length;
      done.value = r.result.items.length;
      successCount.value = r.result.success_count;
      failedCount.value = r.result.failed_count;
      // 节点级状态从 items 反推
      const next: Record<string, NodeRunState> = {};
      for (const it of r.result.items) {
        if (it.node_token) next[it.node_token] = it.status;
      }
      nodeStates.value = next;
    }
    if (r.error) {
      lastError.value = r.error;
    }
  }

  return {
    // state
    stage,
    taskId,
    wikiUrl,
    tree,
    selectedTokens,
    phase,
    total,
    done,
    successCount,
    failedCount,
    currentDoc,
    currentItemType,
    estimatedRemainingSeconds,
    cancelled,
    outputRoot,
    items,
    taskBarVisible,
    nodeStates,
    lastError,
    // getters
    running,
    finished,
    progressPercent,
    phaseIndex,
    phaseLabel,
    selectedBreakdown,
    hasFailure,
    // actions
    scan,
    start,
    cancel,
    reset,
    clearAll,
    applyProgress,
    applyTaskResult,
  };
});

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/** 递归收集所有非 folder 节点的 token（默认全选） */
function collectDocTokens(root: WikiNode): string[] {
  const out: string[] = [];
  const walk = (n: WikiNode) => {
    if (n.obj_type !== "folder") out.push(n.node_token);
    n.children.forEach(walk);
  };
  walk(root);
  return out;
}