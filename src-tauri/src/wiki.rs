//! Wiki 知识库目录树递归遍历
//!
//! 职责：
//! 1. 获取根节点信息（space_id、has_child）
//! 2. 递归遍历子节点，构建完整目录树
//! 3. 保留层级结构（depth、position）
//! 4. 批量提取知识库（按目录顺序逐个提取）

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::extract;
use crate::lark;
use crate::markdown;
use crate::models::{
    DocFailure, ExtractStatus, Settings, WikiExtractResult, WikiNode, WikiNodeType,
};

/// 获取 Wiki 节点树
///
/// 流程：
/// 1. 调 wiki +node-get 获取根节点信息（space_id、has_child）
/// 2. 如果有子节点，调 wiki +node-list 递归遍历
/// 3. 返回完整的 WikiNode 树结构
pub fn get_wiki_tree(wiki_url: &str) -> AppResult<WikiNode> {
    let node_token = extract::parse_node_token(wiki_url);

    // 获取根节点信息
    let root_info = lark::wiki_node_get(&node_token)?;

    let space_id = root_info
        .space_id
        .ok_or_else(|| AppError::LarkCliResponse("无法获取 space_id".to_string()))?;

    let title = root_info.title.clone().unwrap_or_else(|| node_token.clone());
    let has_child = root_info.has_child.unwrap_or(false);
    let obj_type = WikiNodeType::from_str(&root_info.obj_type.clone().unwrap_or_default());

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
        root.children = traverse_children(&space_id, &node_token, 1)?;
    }

    Ok(root)
}

/// 递归遍历子节点
///
/// `depth`: 当前子节点的深度（根节点 depth=0，其子节点 depth=1）
fn traverse_children(space_id: &str, parent_token: &str, depth: usize) -> AppResult<Vec<WikiNode>> {
    let items = lark::wiki_node_list(space_id, parent_token)?;

    let mut nodes: Vec<WikiNode> = items
        .into_iter()
        .enumerate()
        .map(|(idx, item)| {
            let node_token = item.node_token.clone().unwrap_or_default();
            let title = item.title.clone().unwrap_or_else(|| node_token.clone());
            let has_child = item.has_child.unwrap_or(false);
            let obj_type = WikiNodeType::from_str(&item.obj_type.clone().unwrap_or_default());

            WikiNode {
                node_token,
                title,
                obj_type,
                has_child,
                obj_token: item.obj_token.clone(),
                position: item.position.unwrap_or(idx as i64) as usize,
                depth,
                children: vec![],
            }
        })
        .collect();

    // 对有子节点的节点递归遍历
    for node in &mut nodes {
        if node.has_child {
            let children = traverse_children(space_id, &node.node_token, depth + 1)?;
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
    // 获取目录树
    let tree = get_wiki_tree(wiki_url)?;
    let wiki_name = tree.title.clone();

    // 创建知识库根目录
    let wiki_dir = Path::new(output_dir).join(markdown::safe_filename(&wiki_name));
    std::fs::create_dir_all(&wiki_dir)?;

    // 收集所有文档节点（保留目录路径）
    let docs = collect_docs_with_path(&tree, selected_tokens);

    let total = docs.len();
    let mut results = Vec::with_capacity(total);
    let mut failures = Vec::new();
    let mut success_count = 0usize;
    let mut failed_count = 0usize;
    let mut partial_count = 0usize;

    for (node, dir_path) in &docs {
        // 构建文档 URL
        let doc_url = extract::build_wiki_url(&node.node_token);

        // 创建文档所在目录
        let full_dir = wiki_dir.join(dir_path);
        std::fs::create_dir_all(&full_dir)?;

        // 带位置前缀的文件名
        let doc_title = markdown::prefixed_filename(node.position, &node.title);

        // 提取文档
        match extract::extract_doc_with_title_async(
            &doc_url,
            Some(&doc_title),
            full_dir.to_str().unwrap_or(""),
            settings,
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
    }

    Ok(WikiExtractResult {
        wiki_name,
        total,
        success_count,
        failed_count,
        partial_count,
        results,
        failures,
    })
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
    collect_docs_recursive(node, &PathBuf::new(), selected_tokens, &mut docs);
    docs
}

/// 递归收集文档节点
fn collect_docs_recursive<'a>(
    node: &'a WikiNode,
    current_path: &Path,
    selected_tokens: Option<&[String]>,
    docs: &mut Vec<DocWithPath<'a>>,
) {
    // 检查当前节点是否在选中列表中（如果有 selected_tokens）
    let is_selected = match selected_tokens {
        None => true, // 没有限制，全部选中
        Some(tokens) => {
            // 检查该节点或其子树中是否有选中的节点
            is_node_or_descendant_selected(node, tokens)
        }
    };

    if !is_selected {
        return;
    }

    // 如果是文档节点，收集
    if matches!(node.obj_type, WikiNodeType::Doc) {
        // 如果有 selected_tokens，检查当前节点是否被选中
        if let Some(tokens) = selected_tokens {
            if !tokens.contains(&node.node_token) {
                // 当前文档未被选中，跳过
                // 但不 return，因为可能子节点中也不需要（已在 is_selected 中检查）
                // 实际上文档节点不应有子节点，所以直接 return
                return;
            }
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
        collect_docs_recursive(child, &child_path, selected_tokens, docs);
    }
}

/// 检查节点本身或其子树中是否有被选中的节点
fn is_node_or_descendant_selected(node: &WikiNode, selected_tokens: &[String]) -> bool {
    if selected_tokens.contains(&node.node_token) {
        return true;
    }
    for child in &node.children {
        if is_node_or_descendant_selected(child, selected_tokens) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
