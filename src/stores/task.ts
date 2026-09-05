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

import { computed, ref, watch } from "vue";
import { defineStore } from "pinia";
import type {
  ExportableCount,
  ExportItemResult,
  Progress,
  TaskPhase,
  WikiNode,
  WikiNodeType,
  WikiTaskResult,
} from "../api/types";
import { countExportable, getWikiTree } from "../api/wiki";
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
  /** 扫描知识库结构中（防止重复点击 / 用于按钮 loading） */
  const scanning = ref(false);

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

  /** 勾选范围后端统计的真实待导出条数（count_exportable），下载前展示用 */
  const exportableCount = ref<ExportableCount | null>(null);
  /** count_exportable 计算中（勾选密集变化时闪烁防抖用） */
  const counting = ref(false);
  /** count_exportable 失败信息（如：勾选尚未完成扫描） */
  const countError = ref<string | null>(null);

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

  // ---- 勾选计数（真实待导出条数） ----
  const EMPTY_COUNT: ExportableCount = {
    total: 0,
    doc: 0,
    sheet: 0,
    bitable: 0,
    file: 0,
    other: 0,
  };
  let countTimer: ReturnType<typeof setTimeout> | null = null;
  /** 计数请求序号：用于丢弃过期响应（后发请求优先），避免并发统计结果乱序/重复 */
  let countSeq = 0;

  /** 防抖刷新：树上快速勾选/取消时合并为一次请求 */
  function scheduleCountRefresh() {
    if (countTimer) clearTimeout(countTimer);
    countTimer = setTimeout(() => {
      countTimer = null;
      void refreshCount();
    }, 250);
  }

  /**
   * 用后端 count_exportable 统计勾选范围真实会导出的条目数。
   * 不在前端自行推算——前端只有"勾选了哪些节点"，展开逻辑在后端，
   * 自算必然与下载结果不一致。
   */
  async function refreshCount() {
    const seq = ++countSeq;
    if (stage.value !== "tree") {
      if (seq === countSeq) counting.value = false;
      return;
    }
    if (selectedTokens.value.length === 0) {
      if (seq === countSeq) {
        exportableCount.value = { ...EMPTY_COUNT };
        countError.value = null;
        counting.value = false;
      }
      return;
    }
    counting.value = true;
    try {
      const result = await countExportable(selectedTokens.value);
      if (seq === countSeq) {
        exportableCount.value = result;
        countError.value = null;
      }
    } catch (err) {
      if (seq === countSeq) countError.value = String(err);
    } finally {
      if (seq === countSeq) counting.value = false;
    }
  }

  // 勾选变化（含全选/取消/清空）后自动重新统计
  watch(selectedTokens, () => {
    if (stage.value === "tree") {
      scheduleCountRefresh();
    }
  });

  // ---- 动作 ----

  /**
   * 扫描知识库结构。失败写入 lastError（App.vue 全局 toast 提示），不向上抛，
   * 保持 empty/tree 原状态让用户可重试。
   */
  async function scan(url: string) {
    if (scanning.value || stage.value === "running") return; // 防重入
    scanning.value = true;
    lastError.value = null;
    try {
      const node = await getWikiTree(url);
      wikiUrl.value = url;
      tree.value = node;
      selectedTokens.value = collectDocTokens(node);
      stage.value = "tree";
      resetProgressFields();
      // 只调度一次防抖计数：selectedTokens 赋值发生在 stage 切到 tree 之前，
      // 不会触发上面的 watch，需在这里显式调度（代替此前立即 refreshCount，
      // 避免扫描完成后计数请求被执行两遍）。
      scheduleCountRefresh();
    } catch (err) {
      lastError.value = String(err);
      console.error("[task.scan] 扫描失败:", err);
    } finally {
      scanning.value = false;
    }
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
      // 启动瞬间的预估总数：优先用后端展开后的真实条数
      total.value = exportableCount.value?.total ?? selectedTokens.value.length;
      stage.value = "running";
      phase.value = "checking_output";
      // taskId 变化时 useTaskProgress 内部 watch 会处理
      stopProgress = useTaskProgress(taskId);
    } catch (err) {
      lastError.value = String(err);
      stage.value = "tree"; // 回到 tree，让用户重试
    }
  }

  /**
   * 取消任务：请求后端置 cancelled 标志后保持轮询，等任务线程收尾并推进到
   * 终态（cancelled），由 useTaskProgress 调 applyTaskResult 落到结果卡。
   * 不能在这里停轮询——后端是协作式取消，当前环节结束前状态不会变化，
   * 一旦停轮询 UI 会永久卡在"正在取消"，任务条/面板/树全锁死无法收尾。
   */
  async function cancel() {
    cancelled.value = true;
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
    if (countTimer) clearTimeout(countTimer);
    countSeq++; // 丢弃可能仍在途的计数响应
    scanning.value = false;
    stage.value = "empty";
    tree.value = null;
    selectedTokens.value = [];
    wikiUrl.value = "";
    exportableCount.value = null;
    countError.value = null;
    counting.value = false;
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
    if (r.error && !cancelled.value) {
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
    scanning,
    // getters
    running,
    finished,
    progressPercent,
    phaseIndex,
    phaseLabel,
    exportableCount,
    counting,
    countError,
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