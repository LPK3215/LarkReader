//! Wiki 知识库目录树递归遍历
//!
//! 职责：
//! 1. 获取根节点信息（space_id、has_child）
//! 2. 递归遍历子节点，构建完整目录树
//! 3. 保留层级结构（depth、position）
//! 4. 批量提取知识库（按目录顺序逐个提取）

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::error::{AppError, AppResult};
use crate::extract;
use crate::lark;
use crate::markdown;
use crate::models::{
    DocFailure, ExportItemResult, ExportItemStatus, ExportableCount, ExtractStatus, Progress,
    Settings, SkippedNode, SpecialExport, SpecialExportFailure, TaskPhase, TaskStatus,
    WikiExtractResult, WikiNode, WikiNodeType,
};

const MAX_WIKI_DEPTH: usize = 64;
const MAX_WIKI_NODES: usize = 10_000;

/// 单篇文档提取失败后的额外自动重试次数（含首次共 3 次尝试）。
///
/// 背景：批量导出遇到的多是 lark-cli / 网络的瞬时错误（抖动、限流、超时），
/// 失败即记录会让用户不得不整库重下。此处对可重试类错误做有限次自动重试，
/// 退避间隔短、有取消检查，永久性错误（未登录、文件系统等）不会进入重试。
const DOC_RETRY_LIMIT: usize = 2;

/// 是否属于「瞬时失败值得自动重试」的错误
fn is_transient_error(error: &AppError) -> bool {
    matches!(
        error.code(),
        // lark-cli 执行/响应解析/网络/超时/任务状态不可用/提取异常：多为瞬时可恢复
        "LARK_CLI_ERROR"
            | "INVALID_CLI_RESPONSE"
            | "NETWORK_ERROR"
            | "COMMAND_TIMEOUT"
            | "STATE_UNAVAILABLE"
            | "EXTRACT_ERROR"
    )
}

/// 第 n 次重试前的退避等待：800ms / 1.6s / 3.2s，封顶 3.2s
fn retry_backoff_ms(attempt: usize) -> u64 {
    [800, 1600, 3200]
        .get(attempt.saturating_sub(1))
        .copied()
        .unwrap_or(3200)
}

/// 扫描模式
///
/// - `Auto`：只导出传入 URL 对应的节点及其子树。默认行为，不变。
/// - `FullSpace`：如果传入节点没有子节点，自动展开整个知识库
///   （列出 space 下全部顶层节点，逐个递归）。Auto 的超集——Auto 能拿到的
///   FullSpace 全能拿到，Auto 拿不到的兄弟节点 FullSpace 也能拿到。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanMode {
    #[default]
    Auto,
    FullSpace,
}

/// 获取 Wiki 节点树（Auto 模式，只导出传入节点及其子树）
///
/// 流程：
/// 1. 调 wiki +node-get 获取根节点信息（space_id、has_child）
/// 2. 如果有子节点，调 wiki +node-list 递归遍历
/// 3. 返回完整的 WikiNode 树结构
pub fn get_wiki_tree(wiki_url: &str) -> AppResult<WikiNode> {
    get_wiki_tree_with_mode(wiki_url, ScanMode::Auto)
}

/// 获取 Wiki 节点树（支持扫描模式选择）
///
/// - `ScanMode::Auto`：等同 `get_wiki_tree`，只导出传入节点及其子树
/// - `ScanMode::FullSpace`：Auto 模式超集。先走 Auto 逻辑，如果传入节点
///   `has_child=false`（无子树），额外调 `wiki +node-list --space-id`（不带
///   parent）列出 space 下全部顶层节点，构造虚拟 Folder 根逐个递归。
///   虚拟根 obj_type=Folder 不会被收集为文档；space 顶层节点按 Auto 模式同款
///   depth=1 挂到虚拟根下，目录结构与 Auto 模式一致。
pub fn get_wiki_tree_with_mode(wiki_url: &str, mode: ScanMode) -> AppResult<WikiNode> {
    let node_token = extract::parse_node_token(wiki_url);
    if node_token.is_empty() {
        return Err(AppError::InvalidInput(
            "Wiki 链接或 token 不能为空".to_string(),
        ));
    }

    // 获取根节点信息
    let root_info = lark::wiki_node_get(&node_token)?;

    let space_id = root_info
        .space_id
        .ok_or_else(|| AppError::LarkCliResponse("无法获取 space_id".to_string()))?;

    let title = root_info
        .title
        .clone()
        .unwrap_or_else(|| node_token.clone());
    let has_child = root_info.has_child.unwrap_or(false);
    let obj_type = WikiNodeType::from_api_value(&root_info.obj_type.clone().unwrap_or_default());

    let mut root = WikiNode {
        node_token: node_token.clone(),
        title,
        obj_type,
        has_child,
        obj_token: root_info.obj_token.clone(),
        position: 0,
        depth: 0,
        children: vec![],
    };

    // Auto 或 FullSpace 模式且有子节点：走原有递归逻辑
    if has_child {
        let mut ancestors = HashSet::from([node_token.clone()]);
        let mut node_count = 1usize;
        root.children =
            traverse_children(&space_id, &node_token, 1, &mut ancestors, &mut node_count)?;
    } else if mode == ScanMode::FullSpace {
        // FullSpace fallback：传入节点无子树，展开整个 space
        let space_roots = lark::wiki_space_roots(&space_id)?;
        if !space_roots.is_empty() {
            // 构造虚拟 Folder 根：obj_type=Folder 不会被收集为文档，
            // title 用传入节点的 title（用户看到的是知识库首页名称）。
            let mut virtual_root = WikiNode {
                node_token: node_token.clone(),
                title: root.title.clone(),
                obj_type: WikiNodeType::Folder,
                has_child: true,
                obj_token: None,
                position: 0,
                depth: 0,
                children: vec![],
            };

            let mut ancestors = HashSet::new();
            let mut node_count = 0usize;

            for item in &space_roots {
                let child_token = item.node_token.clone().unwrap_or_default();
                if child_token.is_empty() {
                    continue;
                }
                let child_title = item.title.clone().unwrap_or_else(|| child_token.clone());
                let child_has_child = item.has_child.unwrap_or(false);
                let child_obj_type =
                    WikiNodeType::from_api_value(&item.obj_type.clone().unwrap_or_default());

                let mut child = WikiNode {
                    node_token: child_token.clone(),
                    title: child_title,
                    obj_type: child_obj_type,
                    has_child: child_has_child,
                    obj_token: item.obj_token.clone(),
                    position: item
                        .position
                        .and_then(|p| usize::try_from(p).ok())
                        .unwrap_or(0),
                    // depth=1：与 Auto 模式的顶层节点对齐。collect_docs_recursive
                    // 用“父节点 depth != 0”来决定是否为其子节点创建目录层；
                    // 若这里填 0，顶层文件夹本身不会生成目录，其下文档会整体
                    // 塌陷到知识库根目录，两个文件夹里的同名文档可能互相覆盖。
                    depth: 1,
                    children: vec![],
                };

                if child_has_child {
                    ancestors.insert(child_token.clone());
                    node_count += 1;
                    if node_count > MAX_WIKI_NODES {
                        return Err(AppError::Extract(format!(
                            "Wiki 节点数量超过限制 {}",
                            MAX_WIKI_NODES
                        )));
                    }
                    child.children = traverse_children(
                        &space_id,
                        &child_token,
                        // space 顶层节点 depth=1，其子节点自然从 depth=2 开始，
                        // 与 Auto 模式（根 depth0 -> 顶层 depth1 -> 更深递增）严格一致。
                        2,
                        &mut ancestors,
                        &mut node_count,
                    )?;
                    ancestors.remove(&child_token);
                }

                virtual_root.children.push(child);
            }

            // 按 position 排序，确保顺序与飞书一致
            virtual_root.children.sort_by_key(|n| n.position);

            return Ok(virtual_root);
        }
    }

    Ok(root)
}

/// 递归遍历子节点
///
/// `depth`: 当前子节点的深度（根节点 depth=0，其子节点 depth=1）
fn traverse_children(
    space_id: &str,
    parent_token: &str,
    depth: usize,
    ancestors: &mut HashSet<String>,
    node_count: &mut usize,
) -> AppResult<Vec<WikiNode>> {
    if depth > MAX_WIKI_DEPTH {
        return Err(AppError::Extract(format!(
            "Wiki 目录深度超过限制 {}",
            MAX_WIKI_DEPTH
        )));
    }
    let items = lark::wiki_node_list(space_id, parent_token)?;

    let mut nodes: Vec<WikiNode> = items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let node_token = item.node_token.clone().unwrap_or_default();
            let title = item.title.clone().unwrap_or_else(|| node_token.clone());
            let has_child = item.has_child.unwrap_or(false);
            let obj_type = WikiNodeType::from_api_value(&item.obj_type.clone().unwrap_or_default());

            WikiNode {
                node_token,
                title,
                obj_type,
                has_child,
                obj_token: item.obj_token.clone(),
                position: item
                    .position
                    .and_then(|p| usize::try_from(p).ok())
                    .unwrap_or(idx),
                depth,
                children: vec![],
            }
        })
        .collect();

    // 对有子节点的节点递归遍历
    for node in &mut nodes {
        if node.node_token.is_empty() {
            return Err(AppError::LarkCliResponse(
                "Wiki 节点缺少 node_token".to_string(),
            ));
        }
        if ancestors.contains(&node.node_token) {
            return Err(AppError::Extract(format!(
                "检测到循环 Wiki 节点: {}",
                node.node_token
            )));
        }
        *node_count += 1;
        if *node_count > MAX_WIKI_NODES {
            return Err(AppError::Extract(format!(
                "Wiki 节点数量超过限制 {}",
                MAX_WIKI_NODES
            )));
        }
        if node.has_child {
            ancestors.insert(node.node_token.clone());
            let children =
                traverse_children(space_id, &node.node_token, depth + 1, ancestors, node_count)?;
            ancestors.remove(&node.node_token);
            node.children = children;
        }
    }

    // 按 position 排序，确保顺序与飞书一致
    nodes.sort_by_key(|n| n.position);

    Ok(nodes)
}

/// 批量提取知识库
///
/// 默认提取全部文档节点。
/// 如果传入 `selected_tokens`，只提取指定节点（保留目录结构）。
///
/// 流程：
/// 1. 获取 Wiki 树
/// 2. 收集所有文档节点
/// 3. 按目录顺序逐个提取（每个文档保存到对应目录层级）
pub async fn extract_wiki(
    wiki_url: &str,
    output_dir: &str,
    settings: &Settings,
    selected_tokens: Option<&[String]>,
) -> AppResult<WikiExtractResult> {
    extract_wiki_controlled(wiki_url, output_dir, settings, selected_tokens, None, None).await
}

pub async fn extract_wiki_controlled(
    wiki_url: &str,
    output_dir: &str,
    settings: &Settings,
    selected_tokens: Option<&[String]>,
    progress: Option<Arc<Mutex<Progress>>>,
    cancelled: Option<Arc<AtomicBool>>,
) -> AppResult<WikiExtractResult> {
    let tree = get_wiki_tree(wiki_url)?;
    extract_wiki_tree_controlled(
        tree,
        output_dir,
        settings,
        selected_tokens,
        progress,
        cancelled,
    )
    .await
}

pub async fn extract_wiki_tree_controlled(
    tree: WikiNode,
    output_dir: &str,
    settings: &Settings,
    selected_tokens: Option<&[String]>,
    progress: Option<Arc<Mutex<Progress>>>,
    cancelled: Option<Arc<AtomicBool>>,
) -> AppResult<WikiExtractResult> {
    let wiki_name = tree.title.clone();
    let started_at = std::time::Instant::now();

    // 创建知识库根目录
    let wiki_dir =
        create_unique_directory(Path::new(output_dir), &markdown::safe_filename(&wiki_name))?;
    let output_root = wiki_dir.to_string_lossy().to_string();

    // 收集所有文档节点（保留目录路径）
    let docs = collect_docs_with_path(&tree, selected_tokens);
    let relevant = selected_tokens.map(|tokens| build_relevant_tokens(&tree, tokens));
    let special_nodes = collect_special_nodes(&tree, selected_tokens, relevant.as_ref());
    let mut skipped = Vec::new();

    let total = docs.len() + special_nodes.len();
    crate::logger::info(format!(
        "知识库「{}」导出开始：文档 {} 篇，特殊资源 {} 个，共 {} 项，输出目录 {}",
        wiki_name,
        docs.len(),
        special_nodes.len(),
        total,
        output_root
    ));
    if let Some(progress) = &progress {
        let mut progress = progress
            .lock()
            .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
        progress.total = total;
        progress.start_phase(TaskPhase::ExportingDocument);
    }
    let mut results = Vec::with_capacity(total);
    let mut failures = Vec::new();
    let mut success_count = 0usize;
    let mut failed_count = 0usize;
    let mut partial_count = 0usize;
    let mut exports = Vec::new();
    let mut export_failures = Vec::new();

    for (doc_idx, (node, dir_path)) in docs.iter().enumerate() {
        if cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            if let Some(progress) = &progress {
                progress
                    .lock()
                    .map_err(|e| AppError::StateUnavailable(e.to_string()))?
                    .status = TaskStatus::Cancelled;
            }
            break;
        }
        // 构建文档 URL
        let doc_url = extract::build_wiki_url(&node.node_token);

        // 创建文档所在目录
        let full_dir = wiki_dir.join(dir_path);
        std::fs::create_dir_all(&full_dir)?;

        // 带位置前缀的文件名
        let doc_title = markdown::prefixed_filename(node.position, &node.title);
        if let Some(progress) = &progress {
            let mut progress = progress
                .lock()
                .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
            progress.current_doc = Some(node.title.clone());
            progress.current_path = Some(full_dir.to_string_lossy().to_string());
            progress.current_item_type = Some(WikiNodeType::Doc);
            progress.phase = TaskPhase::ExportingDocument;
        }

        // 提取文档（对可重试的瞬时失败自动重试，见 DOC_RETRY_LIMIT）
        let item_started = std::time::Instant::now();
        let mut retried = 0usize;
        let outcome = loop {
            let attempt = extract::extract_doc_with_title_async_controlled(
                &doc_url,
                Some(&doc_title),
                full_dir.to_str().unwrap_or(""),
                settings,
                cancelled.clone(),
            )
            .await;
            if attempt.is_ok() || retried >= DOC_RETRY_LIMIT {
                break attempt;
            }
            // 任务已取消或属永久性错误：不再浪费时间重试
            if cancelled
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                break attempt;
            }
            let error = attempt.as_ref().expect_err("is_ok 已排除");
            if !is_transient_error(error) {
                break attempt;
            }
            retried += 1;
            crate::logger::warn(format!(
                "[{}/{}] 导出文档「{}」失败，{}ms 后第 {}/{} 次自动重试：{}",
                doc_idx + 1,
                docs.len(),
                node.title,
                retry_backoff_ms(retried),
                retried,
                DOC_RETRY_LIMIT,
                error
            ));
            tokio::time::sleep(std::time::Duration::from_millis(retry_backoff_ms(retried))).await;
        };

        // 用户在本篇导出期间点了取消：这篇既不算成功也不算失败。
        // 若继续统计，被打断的图片下载会把它记成"部分成功"，与用户
        // 看到的"任务已取消"自相矛盾。这里直接跳出，结果里只保留已完成的部分。
        if cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            crate::logger::info(format!(
                "任务已取消，文档「{}」中止导出，不计入结果",
                node.title
            ));
            break;
        }

        match outcome {
            Ok(result) => {
                let item_ms = item_started.elapsed().as_millis();
                match result.status {
                    ExtractStatus::Success => {
                        success_count += 1;
                        crate::logger::info(format!(
                            "[{}/{}] 导出文档「{}」：成功（{}ms）",
                            doc_idx + 1,
                            docs.len(),
                            node.title,
                            item_ms
                        ));
                    }
                    ExtractStatus::Partial => {
                        partial_count += 1;
                        crate::logger::warn(format!(
                            "[{}/{}] 导出文档「{}」：部分成功（{}ms）{}",
                            doc_idx + 1,
                            docs.len(),
                            node.title,
                            item_ms,
                            result
                                .errors
                                .first()
                                .map(|error| format!("，原因：{error}"))
                                .unwrap_or_default()
                        ));
                    }
                    ExtractStatus::Failed => {
                        failed_count += 1;
                        crate::logger::warn(format!(
                            "[{}/{}] 导出文档「{}」：失败（{}ms）",
                            doc_idx + 1,
                            docs.len(),
                            node.title,
                            item_ms
                        ));
                    }
                }
                results.push(result);
            }
            Err(error) => {
                failed_count += 1;
                let message = error.to_string();
                crate::logger::error(format!(
                    "[{}/{}] 导出文档「{}」失败{}：{}",
                    doc_idx + 1,
                    docs.len(),
                    node.title,
                    if retried > 0 {
                        format!("（自动重试 {retried} 次后仍失败）")
                    } else {
                        String::new()
                    },
                    message
                ));
                let failure = DocFailure {
                    title: node.title.clone(),
                    node_token: node.node_token.clone(),
                    error: if retried > 0 {
                        format!("自动重试 {retried} 次后仍失败：{message}")
                    } else {
                        message
                    },
                };
                failures.push(failure);
            }
        }
        if let Some(progress) = &progress {
            let mut progress = progress
                .lock()
                .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
            progress.done += 1;
            progress.success_count = success_count + partial_count;
            progress.failed_count = failed_count;
            progress.errors = failures
                .iter()
                .map(|failure| failure.error.clone())
                .collect();
            progress.refresh_timing();
        }
    }

    for (spec_idx, node) in special_nodes.into_iter().enumerate() {
        if cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed))
        {
            break;
        }
        let safe = markdown::prefixed_filename(node.position, &node.title);
        if let Some(progress) = &progress {
            let mut progress = progress
                .lock()
                .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
            progress.current_doc = Some(node.title.clone());
            progress.current_path = Some(wiki_dir.to_string_lossy().to_string());
            progress.current_item_type = Some(node.obj_type.clone());
            progress.phase = match node.obj_type {
                WikiNodeType::Sheet => TaskPhase::ExportingSheet,
                WikiNodeType::Bitable => TaskPhase::ExportingBitable,
                WikiNodeType::File => TaskPhase::ExportingFile,
                _ => TaskPhase::Finalizing,
            };
        }
        let result: AppResult<Vec<String>> = match &node.obj_type {
            WikiNodeType::Sheet => {
                let path = wiki_dir.join(format!("{}.xlsx", safe));
                lark::sheets_export_controlled(
                    &extract::build_wiki_url(&node.node_token),
                    &path.to_string_lossy(),
                    cancelled.as_deref(),
                )
                .map(|saved| vec![saved])
            }
            WikiNodeType::Bitable => {
                export_bitable(node, &wiki_dir.join(&safe), cancelled.as_deref())
            }
            WikiNodeType::File => {
                // 挂载在 Wiki 上的普通文件（zip/pdf/…）：用 Drive 预览接口取原文件。
                // obj_token 即 Drive file token；个别缺失时退化为 node_token 再试。
                let file_token = node.obj_token.as_deref().unwrap_or(&node.node_token);
                let path = wiki_dir.join(&safe);
                lark::drive_file_preview_controlled(
                    file_token,
                    &path.to_string_lossy(),
                    cancelled.as_deref(),
                )
                .map(|saved| vec![saved])
            }
            other => {
                let reason = format!("当前版本暂不支持该节点类型: {:?}", other);
                skipped.push(SkippedNode {
                    title: node.title.clone(),
                    node_token: node.node_token.clone(),
                    obj_type: other.clone(),
                    reason: reason.clone(),
                });
                crate::logger::warn(format!(
                    "[{}/{}] 跳过「{}」（{}）：{}",
                    spec_idx + 1,
                    total,
                    node.title,
                    node_type_label(other),
                    reason
                ));
                if let Some(progress) = &progress {
                    let mut progress = progress
                        .lock()
                        .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
                    progress.done += 1;
                    progress.refresh_timing();
                }
                continue;
            }
        };
        match result {
            Ok(paths) => {
                success_count += 1;
                crate::logger::info(format!(
                    "[{}/{}] 导出「{}」（{}）成功 → {}",
                    spec_idx + 1,
                    total,
                    node.title,
                    node_type_label(&node.obj_type),
                    paths.join("；")
                ));
                exports.push(SpecialExport {
                    title: node.title.clone(),
                    node_token: node.node_token.clone(),
                    obj_type: node.obj_type.clone(),
                    paths,
                });
            }
            Err(error) => {
                failed_count += 1;
                crate::logger::error(format!(
                    "[{}/{}] 导出「{}」（{}）失败：{}",
                    spec_idx + 1,
                    total,
                    node.title,
                    node_type_label(&node.obj_type),
                    error
                ));
                export_failures.push(SpecialExportFailure {
                    title: node.title.clone(),
                    node_token: node.node_token.clone(),
                    obj_type: node.obj_type.clone(),
                    error: error.to_string(),
                });
            }
        }
        if let Some(progress) = &progress {
            let mut progress = progress
                .lock()
                .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
            progress.done += 1;
            progress.success_count = success_count + partial_count;
            progress.failed_count = failed_count;
            progress.refresh_timing();
        }
    }
    let skipped_count = skipped.len();
    let completed_count =
        results.len() + failures.len() + exports.len() + export_failures.len() + skipped.len();
    let mut items = Vec::with_capacity(completed_count);
    items.extend(results.iter().map(|result| ExportItemResult {
        title: result.title.clone(),
        node_token: None,
        obj_type: WikiNodeType::Doc,
        status: match result.status {
            ExtractStatus::Success => ExportItemStatus::Success,
            ExtractStatus::Partial => ExportItemStatus::Partial,
            ExtractStatus::Failed => ExportItemStatus::Failed,
        },
        paths: vec![result.filepath.clone()],
        message: (!result.errors.is_empty()).then(|| result.errors.join("; ")),
    }));
    items.extend(failures.iter().map(|failure| ExportItemResult {
        title: failure.title.clone(),
        node_token: Some(failure.node_token.clone()),
        obj_type: WikiNodeType::Doc,
        status: ExportItemStatus::Failed,
        paths: vec![],
        message: Some(failure.error.clone()),
    }));
    items.extend(exports.iter().map(|export| ExportItemResult {
        title: export.title.clone(),
        node_token: Some(export.node_token.clone()),
        obj_type: export.obj_type.clone(),
        status: ExportItemStatus::Success,
        paths: export.paths.clone(),
        message: None,
    }));
    items.extend(export_failures.iter().map(|failure| ExportItemResult {
        title: failure.title.clone(),
        node_token: Some(failure.node_token.clone()),
        obj_type: failure.obj_type.clone(),
        status: ExportItemStatus::Failed,
        paths: vec![],
        message: Some(failure.error.clone()),
    }));
    items.extend(skipped.iter().map(|item| ExportItemResult {
        title: item.title.clone(),
        node_token: Some(item.node_token.clone()),
        obj_type: item.obj_type.clone(),
        status: ExportItemStatus::Skipped,
        paths: vec![],
        message: Some(item.reason.clone()),
    }));

    if let Some(progress) = &progress {
        let mut progress = progress
            .lock()
            .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
        progress.phase = TaskPhase::Finalizing;
    }

    crate::logger::info(format!(
        "知识库「{}」导出结束：成功 {}，失败 {}，部分 {}，跳过 {}（共 {} 项），用时 {}ms，输出目录 {}",
        wiki_name,
        success_count,
        failed_count,
        partial_count,
        skipped_count,
        total,
        started_at.elapsed().as_millis(),
        output_root
    ));

    Ok(WikiExtractResult {
        wiki_name,
        output_root,
        total,
        success_count,
        failed_count,
        partial_count,
        results,
        failures,
        skipped_count,
        skipped,
        exports,
        export_failures,
        cancelled: cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Relaxed)),
        completed_count,
        items,
    })
}

fn create_unique_directory(parent: &Path, name: &str) -> AppResult<PathBuf> {
    std::fs::create_dir_all(parent)?;
    for suffix in 1..=10_000 {
        let directory_name = if suffix == 1 {
            name.to_string()
        } else {
            format!("{} ({})", name, suffix)
        };
        let candidate = parent.join(directory_name);
        match std::fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(AppError::Extract(
        "无法创建不冲突的知识库导出目录".to_string(),
    ))
}

fn collect_special_nodes<'a>(
    node: &'a WikiNode,
    selected_tokens: Option<&[String]>,
    relevant_tokens: Option<&HashSet<String>>,
) -> Vec<&'a WikiNode> {
    let mut nodes = Vec::new();
    collect_special_recursive(node, selected_tokens, relevant_tokens, false, &mut nodes);
    nodes
}

fn collect_special_recursive<'a>(
    node: &'a WikiNode,
    selected_tokens: Option<&[String]>,
    relevant_tokens: Option<&HashSet<String>>,
    ancestor_selected: bool,
    nodes: &mut Vec<&'a WikiNode>,
) {
    let directly_selected = selected_tokens
        .map(|tokens| tokens.contains(&node.node_token))
        .unwrap_or(true);
    let selected = ancestor_selected || directly_selected;
    let relevant = selected_tokens.is_none()
        || selected
        || relevant_tokens.is_some_and(|tokens| tokens.contains(&node.node_token));
    if !relevant {
        return;
    }
    if selected
        && matches!(
            node.obj_type,
            WikiNodeType::Sheet | WikiNodeType::Bitable | WikiNodeType::File | WikiNodeType::Other
        )
    {
        nodes.push(node);
    }
    for child in &node.children {
        collect_special_recursive(child, selected_tokens, relevant_tokens, selected, nodes);
    }
}

fn export_bitable(
    node: &WikiNode,
    output_dir: &Path,
    cancelled: Option<&AtomicBool>,
) -> AppResult<Vec<String>> {
    let base_token = node.obj_token.as_deref().unwrap_or(&node.node_token);
    let mut paths = Vec::new();
    std::fs::create_dir_all(output_dir)?;
    let data = lark::base_table_list_controlled(base_token, cancelled)?;
    let tables = data
        .get("items")
        .or_else(|| data.get("tables"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::LarkCliResponse("无法解析多维表格的数据表列表".to_string()))?;
    for (index, table) in tables.iter().enumerate() {
        let id = table
            .get("table_id")
            .or_else(|| table.get("id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::LarkCliResponse("数据表缺少 table_id".to_string()))?;
        let name = table.get("name").and_then(|v| v.as_str()).unwrap_or(id);
        let filename = format!("{:02}_{}.ndjson", index + 1, markdown::safe_filename(name));
        let path = output_dir.join(filename);
        lark::base_records_export_controlled(base_token, id, &path.to_string_lossy(), cancelled)?;
        paths.push(path.to_string_lossy().to_string());
    }
    Ok(paths)
}

/// 统计勾选范围内**真实会被导出**的条目数（按节点类型分组）
///
/// 与"勾选了多少个节点"不是一回事：勾选一个父节点会展开成它的全部可导出
/// 后代。本函数与 `extract_wiki_tree_controlled` 用同一套收集逻辑，保证
/// 下载前展示的预估条数与任务进度里的 `total` 口径一致。
pub fn count_exportable_breakdown(
    tree: &WikiNode,
    selected_tokens: Option<&[String]>,
) -> ExportableCount {
    let relevant = selected_tokens.map(|tokens| build_relevant_tokens(tree, tokens));
    let mut count = ExportableCount {
        total: 0,
        doc: collect_docs_with_path(tree, selected_tokens).len(),
        sheet: 0,
        bitable: 0,
        file: 0,
        other: 0,
    };
    for node in collect_special_nodes(tree, selected_tokens, relevant.as_ref()) {
        match node.obj_type {
            WikiNodeType::Sheet => count.sheet += 1,
            WikiNodeType::Bitable => count.bitable += 1,
            WikiNodeType::File => count.file += 1,
            _ => count.other += 1,
        }
    }
    count.total = count.doc + count.sheet + count.bitable + count.file + count.other;
    count
}

/// 节点类型的中文描述（仅用于日志）
fn node_type_label(node_type: &WikiNodeType) -> &'static str {
    match node_type {
        WikiNodeType::Doc => "文档",
        WikiNodeType::Sheet => "表格",
        WikiNodeType::Bitable => "多维表格",
        WikiNodeType::File => "文件",
        WikiNodeType::Folder => "目录",
        WikiNodeType::Other => "其他",
    }
}

/// 文档节点及其在目录树中的相对路径
type DocWithPath<'a> = (&'a WikiNode, PathBuf);

/// 收集所有文档节点，同时记录每个文档的目录路径
///
/// 返回 Vec<(node, relative_dir_path)>
/// relative_dir_path 是相对于知识库根目录的路径（如 "01_子目录/02_二级目录"）
fn collect_docs_with_path<'a>(
    node: &'a WikiNode,
    selected_tokens: Option<&[String]>,
) -> Vec<DocWithPath<'a>> {
    let mut docs = Vec::new();
    let relevant = selected_tokens.map(|tokens| build_relevant_tokens(node, tokens));
    collect_docs_recursive(
        node,
        &PathBuf::new(),
        selected_tokens,
        relevant.as_ref(),
        false,
        &mut docs,
    );
    docs
}

fn build_relevant_tokens(node: &WikiNode, selected_tokens: &[String]) -> HashSet<String> {
    fn visit(node: &WikiNode, selected: &HashSet<&str>, relevant: &mut HashSet<String>) -> bool {
        let mut contains_selected = selected.contains(node.node_token.as_str());
        for child in &node.children {
            contains_selected |= visit(child, selected, relevant);
        }
        if contains_selected {
            relevant.insert(node.node_token.clone());
        }
        contains_selected
    }
    let selected: HashSet<&str> = selected_tokens.iter().map(String::as_str).collect();
    let mut relevant = HashSet::new();
    visit(node, &selected, &mut relevant);
    relevant
}

/// 递归收集文档节点
fn collect_docs_recursive<'a>(
    node: &'a WikiNode,
    current_path: &Path,
    selected_tokens: Option<&[String]>,
    relevant_tokens: Option<&HashSet<String>>,
    ancestor_selected: bool,
    docs: &mut Vec<DocWithPath<'a>>,
) {
    let directly_selected = selected_tokens
        .map(|tokens| tokens.contains(&node.node_token))
        .unwrap_or(true);
    let subtree_selected = ancestor_selected || directly_selected;
    // 检查当前节点是否在选中列表中（如果有 selected_tokens）
    let is_selected = selected_tokens.is_none()
        || subtree_selected
        || relevant_tokens.is_some_and(|tokens| tokens.contains(&node.node_token));

    if !is_selected {
        return;
    }

    // 文档节点：收集本体；若它同时带子节点（父文档 + 子页面），不能在此 return，
    // 否则挂在文档节点下的所有子文档会整体丢失。
    if matches!(node.obj_type, WikiNodeType::Doc) && (selected_tokens.is_none() || subtree_selected)
    {
        docs.push((node, current_path.to_path_buf()));
    }

    // 文件夹节点或带子页面的文档节点：递归处理子节点
    for child in &node.children {
        let child_path = if matches!(node.depth, 0) {
            // 根节点的子节点不需要额外目录层
            current_path.to_path_buf()
        } else {
            // 非根节点的节点（文件夹或父文档），创建子目录
            current_path.join(markdown::prefixed_filename(node.position, &node.title))
        };
        collect_docs_recursive(
            child,
            &child_path,
            selected_tokens,
            relevant_tokens,
            subtree_selected,
            docs,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_docs_with_path, collect_special_nodes, count_exportable_breakdown};
    use crate::models::{WikiNode, WikiNodeType};
    use std::path::PathBuf;

    #[test]
    fn test_count_docs() {
        let tree = WikiNode {
            node_token: "root".to_string(),
            title: "root".to_string(),
            obj_type: WikiNodeType::Folder,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 0,
            children: vec![
                WikiNode {
                    node_token: "doc1".to_string(),
                    title: "doc1".to_string(),
                    obj_type: WikiNodeType::Doc,
                    has_child: false,
                    obj_token: Some("obj1".to_string()),
                    position: 0,
                    depth: 1,
                    children: vec![],
                },
                WikiNode {
                    node_token: "folder1".to_string(),
                    title: "folder1".to_string(),
                    obj_type: WikiNodeType::Folder,
                    has_child: true,
                    obj_token: None,
                    position: 1,
                    depth: 1,
                    children: vec![WikiNode {
                        node_token: "doc2".to_string(),
                        title: "doc2".to_string(),
                        obj_type: WikiNodeType::Doc,
                        has_child: false,
                        obj_token: Some("obj2".to_string()),
                        position: 0,
                        depth: 2,
                        children: vec![],
                    }],
                },
            ],
        };

        assert_eq!(tree.count_docs(), 2);
    }

    #[test]
    fn selecting_folder_includes_descendant_documents() {
        let doc = WikiNode {
            node_token: "doc".into(),
            title: "Doc".into(),
            obj_type: WikiNodeType::Doc,
            has_child: false,
            obj_token: None,
            position: 0,
            depth: 2,
            children: vec![],
        };
        let folder = WikiNode {
            node_token: "folder".into(),
            title: "Folder".into(),
            obj_type: WikiNodeType::Folder,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 1,
            children: vec![doc],
        };
        let root = WikiNode {
            node_token: "root".into(),
            title: "Root".into(),
            obj_type: WikiNodeType::Folder,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 0,
            children: vec![folder],
        };
        let selected = vec!["folder".to_string()];
        let docs = collect_docs_with_path(&root, Some(&selected));
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].0.node_token, "doc");
    }

    #[test]
    fn doc_parent_with_child_pages_collects_all_descendants() {
        // 复现真实缺陷：父节点是文档（Doc）且带子文档（子页面）时，
        // 收集器曾只导出父文档、丢弃全部子文档。
        let doc_b = WikiNode {
            node_token: "docB".into(),
            title: "DocB".into(),
            obj_type: WikiNodeType::Doc,
            has_child: false,
            obj_token: None,
            position: 0,
            depth: 2,
            children: vec![],
        };
        let doc_a = WikiNode {
            node_token: "docA".into(),
            title: "DocA".into(),
            obj_type: WikiNodeType::Doc,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 1,
            children: vec![doc_b],
        };
        let root = WikiNode {
            node_token: "root".into(),
            title: "Root".into(),
            obj_type: WikiNodeType::Folder,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 0,
            children: vec![doc_a],
        };

        // 全量导出：父文档与其子页面都应被收集
        let docs = collect_docs_with_path(&root, None);
        assert_eq!(docs.len(), 2, "父文档 + 子页面应全部被收集");
        assert_eq!(docs[0].0.node_token, "docA");
        assert!(
            docs[0].1.as_os_str().is_empty(),
            "根下文档平铺在知识库根目录"
        );
        assert_eq!(docs[1].0.node_token, "docB");
        assert_eq!(
            docs[1].1.to_string_lossy(),
            "00_DocA",
            "子页面应落在父文档目录下"
        );

        // 只选中父文档：应自动包含其子页面
        let selected = vec!["docA".to_string()];
        let docs = collect_docs_with_path(&root, Some(&selected));
        assert_eq!(docs.len(), 2, "选中父文档应包含其子页面");
    }

    #[test]
    fn file_node_collected_as_special() {
        let file = WikiNode {
            node_token: "fileToken".into(),
            title: "资源包.zip".into(),
            obj_type: WikiNodeType::File,
            has_child: false,
            obj_token: Some("driveToken".into()),
            position: 0,
            depth: 1,
            children: vec![],
        };
        let root = WikiNode {
            node_token: "root".into(),
            title: "Root".into(),
            obj_type: WikiNodeType::Folder,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 0,
            children: vec![file],
        };

        let special = collect_special_nodes(&root, None, None);
        assert_eq!(special.len(), 1);
        assert!(matches!(special[0].obj_type, WikiNodeType::File));
        assert_eq!(special[0].obj_token.as_deref(), Some("driveToken"));

        // 选中不相关的节点时不应收集
        let selected = vec!["other".to_string()];
        let special = collect_special_nodes(&root, Some(&selected), None);
        assert_eq!(special.len(), 0);
    }

    #[test]
    fn full_space_top_level_folder_keeps_directory_layer() {
        // 复现 FullSpace 模式（整库展开）缺陷：space 顶层节点若被误设成 depth=0，
        // collect_docs_recursive 就不会为顶层文件夹创建目录层，其下文档会
        // 全部塌陷到知识库根目录，两个文件夹里的同名文档可能互相覆盖。
        let top_folder = WikiNode {
            node_token: "A".into(),
            title: "指南".into(),
            obj_type: WikiNodeType::Folder,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 1,
            children: vec![
                WikiNode {
                    node_token: "a1".into(),
                    title: "README".into(),
                    obj_type: WikiNodeType::Doc,
                    has_child: false,
                    obj_token: None,
                    position: 0,
                    depth: 2,
                    children: vec![],
                },
                WikiNode {
                    node_token: "G".into(),
                    title: "进阶".into(),
                    obj_type: WikiNodeType::Folder,
                    has_child: true,
                    obj_token: None,
                    position: 1,
                    depth: 2,
                    children: vec![WikiNode {
                        node_token: "a2".into(),
                        title: "约定".into(),
                        obj_type: WikiNodeType::Doc,
                        has_child: false,
                        obj_token: None,
                        position: 0,
                        depth: 3,
                        children: vec![],
                    }],
                },
            ],
        };
        let top_doc = WikiNode {
            node_token: "B".into(),
            title: "首页".into(),
            obj_type: WikiNodeType::Doc,
            has_child: false,
            obj_token: None,
            position: 2,
            depth: 1,
            children: vec![],
        };
        let virtual_root = WikiNode {
            node_token: "root".into(),
            title: "Root".into(),
            obj_type: WikiNodeType::Folder,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 0,
            children: vec![top_folder, top_doc],
        };

        let docs = collect_docs_with_path(&virtual_root, None);
        assert_eq!(docs.len(), 3);
        let by_token: std::collections::HashMap<_, _> = docs
            .into_iter()
            .map(|(n, p)| (n.node_token.clone(), p))
            .collect();
        assert_eq!(
            by_token["a1"],
            PathBuf::from("00_指南"),
            "顶层文件夹内的文档应落在该文件夹目录下"
        );
        assert_eq!(
            by_token["a2"],
            PathBuf::from("00_指南").join("01_进阶"),
            "嵌套文件夹的目录层必须完整保留"
        );
        assert_eq!(
            by_token["B"],
            PathBuf::new(),
            "space 顶层的独立文档仍平铺在知识库根目录"
        );
    }

    fn node(token: &str, obj_type: WikiNodeType, children: Vec<WikiNode>) -> WikiNode {
        WikiNode {
            node_token: token.into(),
            title: token.into(),
            obj_type,
            has_child: !children.is_empty(),
            obj_token: Some(format!("obj-{token}")),
            position: 0,
            depth: 1,
            children,
        }
    }

    #[test]
    fn count_exportable_expands_selected_parent() {
        // 复现真实缺陷：勾选一个父节点，UI 曾只显示"合计 1 项"，
        // 而实际会展开导出它的全部可导出后代。
        let folder = node(
            "folder",
            WikiNodeType::Folder,
            vec![
                node("docA", WikiNodeType::Doc, vec![]),
                node("docB", WikiNodeType::Doc, vec![]),
            ],
        );
        let sheet = node("sheet", WikiNodeType::Sheet, vec![]);
        let bitable = node("bitable", WikiNodeType::Bitable, vec![]);
        let file = node("file", WikiNodeType::File, vec![]);
        let root = WikiNode {
            node_token: "root".into(),
            title: "Root".into(),
            obj_type: WikiNodeType::Folder,
            has_child: true,
            obj_token: None,
            position: 0,
            depth: 0,
            children: vec![folder, sheet, bitable, file],
        };

        // 全量：2 篇文档 + 表格 + 多维表格 + 附件 = 5
        let all = count_exportable_breakdown(&root, None);
        assert_eq!(all.doc, 2);
        assert_eq!(all.sheet, 1);
        assert_eq!(all.bitable, 1);
        assert_eq!(all.file, 1);
        assert_eq!(all.other, 0);
        assert_eq!(all.total, 5);

        // 只勾父文件夹：应展开为其中的 2 篇文档，而不是 1 项
        let selected = vec!["folder".to_string()];
        let only_folder = count_exportable_breakdown(&root, Some(&selected));
        assert_eq!(
            only_folder.doc, 2,
            "勾选父文件夹应展开为其中全部文档，而非计 1 项"
        );
        assert_eq!(only_folder.sheet, 0);
        assert_eq!(only_folder.total, 2);

        // 只勾表格：只算表格
        let selected = vec!["sheet".to_string()];
        let only_sheet = count_exportable_breakdown(&root, Some(&selected));
        assert_eq!(only_sheet.total, 1);
        assert_eq!(only_sheet.sheet, 1);
        assert_eq!(only_sheet.doc, 0);
    }
}
