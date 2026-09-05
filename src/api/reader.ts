// ============================================================================
// src/api/reader.ts —— 本地阅读 IPC
//
// 对应后端命令：
//   list_reader_dir(path)      -> ReaderEntry[]（一层子项，目录优先按名排序）
//   read_reader_md(path)       -> string（.md 文档文本）
//   read_reader_binary(path)   -> ReaderBinary（图片等资源的 data URL）
//   find_first_reader_doc(path)-> string | null（目录树里第一篇 md，应用内阅读直达用）
//
// 纯本地文件系统读取，不依赖飞书登录/网络。「本地阅读」页（/reader）专用。
// ============================================================================

import { invoke } from "@tauri-apps/api/core";
import type { ReaderBinary, ReaderEntry } from "./types";

/** 列出目录的一层子项（惰性加载：展开目录时再调）。 */
export async function listReaderDir(path: string): Promise<ReaderEntry[]> {
  return invoke<ReaderEntry[]>("list_reader_dir", { path });
}

/** 读取 .md 文档文本（渲染正文用）。 */
export async function readReaderMd(path: string): Promise<string> {
  return invoke<string>("read_reader_md", { path });
}

/** 读取二进制资源（图片等），返回 data URL。 */
export async function readReaderBinary(path: string): Promise<ReaderBinary> {
  return invoke<ReaderBinary>("read_reader_binary", { path });
}

/** 在导出目录树里找第一篇 .md（任务历史「应用内阅读」自动打开用）。 */
export async function findFirstReaderDoc(path: string): Promise<string | null> {
  return invoke<string | null>("find_first_reader_doc", { path });
}
