// ============================================================================
// src/api/wiki.ts —— 知识库结构扫描命令封装（结构占位）
//
// 对应后端命令：
//   get_wiki_tree(wiki_url) -> WikiNode  只扫结构不拉正文（预览/勾选用）
//
// 入参：wikiUrl（camelCase）；出参：WikiNode（字段 snake_case）——
//   node_token / title / obj_type / has_child / obj_token / position / depth ...
//   （obj_type 为枚举的 snake_case 字符串，如 doc/sheet/bitable/file/folder）
//
// 填充时机：M2（工作台「粘贴链接 -> 扫树」）实现。
// 约定：树数据直接交付给 components/NodeTree.vue 渲染，不在此做业务处理。
// ============================================================================

export {};
