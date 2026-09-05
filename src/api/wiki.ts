// ============================================================================
// src/api/wiki.ts —— 知识库结构相关 IPC
//
// 全部是 Tauri command 的薄包装，不含业务逻辑。
//   getWikiTree(wikiUrl) → WikiNode（树根，children 是折叠展开的）
//
// 前端 store 只调这里，不直接调 invoke()。
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { ExportableCount, WikiNode } from "./types";

/** 扫描模式：auto=A模式（只导出传入节点子树），full_space=C模式（整库展开） */
export type ScanMode = "auto" | "full_space";

/**
 * 拉取知识库目录树（只摸结构，不拉正文）。
 * 后端走飞书 lark-cli 完成。
 *
 * @param wikiUrl  知识库节点链接
 * @param scanMode 可选，默认 auto。full_space 时无子节点的 URL 会自动展开整个知识库
 * @throws AppError(InvalidInput | Extract | Other) 来自后端
 */
export async function getWikiTree(
  wikiUrl: string,
  scanMode?: ScanMode
): Promise<WikiNode> {
  return invoke<WikiNode>("get_wiki_tree", {
    wikiUrl,
    scanMode: scanMode ?? null,
  });
}

/**
 * 统计勾选范围内真实会被导出的条目数（下载前的预估）。
 * 勾选父节点会展开成其全部可导出后代，因此返回数字通常大于直接勾选的节点数，
 * 与任务进度里的 total 口径一致。后端复用最近一次扫描缓存，不重复拉树。
 */
export async function countExportable(selectedTokens: string[]): Promise<ExportableCount> {
  return invoke<ExportableCount>("count_exportable", { selectedTokens });
}