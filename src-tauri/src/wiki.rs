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
    DocFailure, ExportItemResult, ExportItemStatus, ExtractStatus, Progress, Settings, SkippedNode,
    SpecialExport, SpecialExportFailure, TaskPhase, TaskStatus, WikiExtractResult, WikiNode,
    WikiNodeType,
};

const MAX_WIKI_DEPTH: usize = 64;
const MAX_WIKI_NODES: usize = 10_000;

/// 获取 Wiki 节点树
///
/// 流程：
/// 1. 调 wiki +node-get 获取根节点信息（space_id、has_child）
/// 2. 如果有子节点，调 wiki +node-list 递归遍历
/// 3. 返回完整的 WikiNode 树结构
pub fn get_wiki_tree(wiki_url: &str) -> AppResult<WikiNode> {
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

    // 递归遍历子节点
    if has_child {
        let mut ancestors = HashSet::from([node_token.clone()]);
        let mut node_count = 1usize;
        root.children =
            traverse_children(&space_id, &node_token, 1, &mut ancestors, &mut node_count)?;
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

    for (node, dir_path) in &docs {
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

        // 提取文档
        match extract::extract_doc_with_title_async_controlled(
            &doc_url,
            Some(&doc_title),
            full_dir.to_str().unwrap_or(""),
            settings,
            cancelled.clone(),
        )
        .await
        {
            Ok(result) => {
                match result.status {
                    ExtractStatus::Success => success_count += 1,
                    ExtractStatus::Partial => partial_count += 1,
                    ExtractStatus::Failed => failed_count += 1,
                }
                results.push(result);
            }
            Err(e) => {
                failed_count += 1;
                let failure = DocFailure {
                    title: node.title.clone(),
                    node_token: node.node_token.clone(),
                    error: e.to_string(),
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

    for node in special_nodes {
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
                _ => TaskPhase::Finalizing,
            };
        }
        if matches!(node.obj_type, WikiNodeType::Other) {
            skipped.push(SkippedNode {
                title: node.title.clone(),
                node_token: node.node_token.clone(),
                obj_type: node.obj_type.clone(),
                reason: "当前版本暂不支持该节点类型".to_string(),
            });
            if let Some(progress) = &progress {
                let mut progress = progress
                    .lock()
                    .map_err(|e| AppError::StateUnavailable(e.to_string()))?;
                progress.done += 1;
                progress.refresh_timing();
            }
            continue;
        }
        let result: AppResult<Vec<String>> = match node.obj_type {
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
            _ => unreachable!("非特殊节点不会进入特殊导出循环"),
        };
        match result {
            Ok(paths) => {
                success_count += 1;
                exports.push(SpecialExport {
                    title: node.title.clone(),
                    node_token: node.node_token.clone(),
                    obj_type: node.obj_type.clone(),
                    paths,
                });
            }
            Err(error) => {
                failed_count += 1;
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
            WikiNodeType::Sheet | WikiNodeType::Bitable | WikiNodeType::Other
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

pub fn count_selected_exportable_nodes(
    tree: &WikiNode,
    selected_tokens: Option<&[String]>,
) -> usize {
    let relevant = selected_tokens.map(|tokens| build_relevant_tokens(tree, tokens));
    collect_docs_with_path(tree, selected_tokens).len()
        + collect_special_nodes(tree, selected_tokens, relevant.as_ref()).len()
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

    // 如果是文档节点，收集
    if matches!(node.obj_type, WikiNodeType::Doc) {
        // 如果有 selected_tokens，检查当前节点是否被选中
        if selected_tokens.is_some() && !subtree_selected {
            return;
        }

        docs.push((node, current_path.to_path_buf()));
        return;
    }

    // 如果是文件夹节点，递归处理子节点
    for child in &node.children {
        let child_path = if matches!(node.depth, 0) {
            // 根节点的子节点不需要额外目录层
            current_path.to_path_buf()
        } else {
            // 非根节点的文件夹节点，创建子目录
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
    use super::collect_docs_with_path;
    use crate::models::{WikiNode, WikiNodeType};

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
}
