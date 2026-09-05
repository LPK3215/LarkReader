// ============================================================================
// src/api/task.ts —— 下载任务命令封装（结构占位）
//
// 对应后端命令：
//   start_extract_wiki(wiki_url, output_dir?, selected_tokens?) -> task_id  异步批量
//   extract_wiki(...)           同步批量（一般交给 start_extract_wiki + 轮询，暂不直接用）
//   get_progress(task_id)       -> Progress { task_id, total, done, current_doc,
//                                             current_path, success_count, failed_count, phase }
//   cancel_task(task_id)        -> ()  请求取消（任务自行检查）
//   get_task_result(task_id)    -> WikiTaskResult { task_id, progress, result?, error? }
//   dismiss_task_result(task_id)-> ()  从已完成列表移除一条
//   list_task_history()         -> Vec<WikiTaskResult>  已完成历史
//
// 入参：wikiUrl / outputDir / selectedTokens / taskId（全部 camelCase）。
// 填充时机：M2（工作台启动下载 + 任务面板）实现。
// 约定：进度轮询逻辑放 ../composables/useTaskProgress.ts，此文件只做单次 invoke 封装。
// ============================================================================

export {};
