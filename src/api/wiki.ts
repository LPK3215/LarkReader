// ============================================================================
// src/api/wiki.ts —— 知识库结构相关 IPC
//
// 全部是 Tauri command 的薄包装，不含业务逻辑。
//   getWikiTree(wikiUrl) → WikiNode（树根，children 是折叠展开的）
//
// 前端 store 只调这里，不直接调 invoke()。
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { WikiNode } from "./types";

/**
 * 拉取知识库目录树（只摸结构，不拉正文）。
 * 后端走飞书 lark-cli 完成。
 *
 * @throws AppError(InvalidInput | Extract | Other) 来自后端
 */
export async function getWikiTree(wikiUrl: string): Promise<WikiNode> {
  return invoke<WikiNode>("get_wiki_tree", { wikiUrl });
}