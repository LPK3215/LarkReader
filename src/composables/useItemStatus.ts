// ============================================================================
// src/composables/useItemStatus.ts —— 导出条目状态的共享展示口径
//
// ExportItemResult 的 status（success / partial / failed / skipped）在结果卡与
// 历史页都要翻译成中文 + badge 样式。历史上两份组件各自维护了一套相同映射，
// 容易漂移；现在统一收敛到这里，新增场景直接 import。
// ============================================================================

import type { ExportItemResult, ExportItemStatus } from "../api/types";

export const ITEM_STATUS_TEXT: Record<ExportItemStatus, string> = {
  success: "成功",
  partial: "部分成功",
  failed: "失败",
  skipped: "跳过",
};

export const ITEM_STATUS_CLASS: Record<ExportItemStatus, string> = {
  success: "lr-badge--success",
  partial: "lr-badge--warning",
  failed: "lr-badge--danger",
  skipped: "lr-badge",
};

/** 是否为「需要用户关注原因」的条目（失败 / 部分成功 / 跳过） */
export function isProblemItem(item: ExportItemResult): boolean {
  return item.status !== "success";
}

export function problemItemsOf(items: ExportItemResult[]): ExportItemResult[] {
  return items.filter(isProblemItem);
}
