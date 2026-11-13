//! 实测画板导出（需登录 lark-cli）：
//! - 内嵌画板：文档正文里的 `<whiteboard>` 标签应导出预览图到 `*_images/wb_tag_XX.jpg`，
//!   正文替换为 `![画板](...)` 链接。
//! - 独立画板：wiki 节点 obj_type=board/whiteboard 应作为特殊节点导出。
//! - 输出目录 `e2e_download_tmp_board/<stage>/`（见 .gitignore），运行前清空。

use lark_reader_lib::{
    extract,
    models::{Settings, WikiNode, WikiNodeType},
    wiki::{extract_wiki, extract_wiki_tree_controlled},
};
use std::path::Path;

const DOC_URL: &str = "https://qcny2iztd1p8.feishu.cn/docx/Yt14dJ8HdoDOUOxXQqucWtL1nod"; // 带 2 个内嵌画板的测试文档
const BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../e2e_download_tmp_board");

#[tokio::test]
async fn board_embedded_export() {
    let settings = Settings::default();
    let stage = format!("{}/doc", BASE);
    reset(&stage);
    match extract::extract_doc_async(DOC_URL, &stage, &settings).await {
        Ok(r) => {
            println!(
                "[doc] {}  图 {}/{} (失败 {})  字数 {}  错误: {:?}",
                r.filename,
                r.images_downloaded,
                r.image_count,
                r.images_failed,
                r.char_count,
                r.errors
            );

            println!("===== 目录树 stage=doc =====");
            print_tree(Path::new(&stage), 0);

            // 断言 1：无错误
            assert!(
                r.errors.is_empty(),
                "画板文档导出不应有错误: {:?}",
                r.errors
            );
            assert_eq!(r.images_failed, 0, "画板预览图下载不应失败");

            // 断言 2：正文中的内嵌画板被替换成 ![画板] 链接 + 原始源码块
            let all_md = collect_text_files(Path::new(&stage), "md");
            let hit = all_md
                .iter()
                .find(|(_, c)| c.contains("![画板]") && c.contains("```mermaid"))
                .cloned();
            let Some((md_path, md_content)) = hit else {
                panic!("没有 .md 同时包含 ![画板] 与 ```mermaid 源码块，画板转换不完整");
            };
            println!("含画板链接+源码的正文: {}", md_path.display());
            assert!(
                md_content.contains("flowchart TD") && md_content.contains("sequenceDiagram"),
                "源码块应保留原始 mermaid 内容"
            );

            // 断言 3：画板预览图落盘且非空（本测试文档含 2 个画板块）
            let boards = collect_files(Path::new(&stage), &|p: &Path| {
                p.file_name()
                    .map(|n| {
                        let name = n.to_string_lossy();
                        name.starts_with("wb_tag_")
                            && (name.ends_with(".jpg") || name.ends_with(".png"))
                    })
                    .unwrap_or(false)
            });
            println!("画板预览图: {} 张", boards.len());
            for p in &boards {
                let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                println!("  {} ({} bytes)", p.display(), size);
                assert!(
                    size > 1024,
                    "画板预览图过小: {} ({} bytes)",
                    p.display(),
                    size
                );
            }
            assert_eq!(boards.len(), 2, "应恰好导出 2 张画板预览图");

            // 断言 4：每个画板都要有独立源码文件（.mmd/.puml），且内容与图片一一对应
            let sources = collect_files(Path::new(&stage), &|p: &Path| {
                p.file_name()
                    .map(|n| {
                        let name = n.to_string_lossy();
                        name.starts_with("wb_tag_")
                            && (name.ends_with(".mmd") || name.ends_with(".puml"))
                    })
                    .unwrap_or(false)
            });
            println!("画板源码文件: {} 份", sources.len());
            let mut source_texts = Vec::new();
            for p in &sources {
                let text = std::fs::read_to_string(p).expect("源码文件应可读");
                println!("  {} => {}", p.display(), text.replace('\n', " / "));
                assert!(!text.trim().is_empty(), "源码文件为空: {}", p.display());
                source_texts.push(text);
            }
            assert_eq!(sources.len(), 2, "应恰好导出 2 份画板源码");
            assert!(
                source_texts.iter().any(|t| t.contains("flowchart TD")),
                "源码文件应保留 flowchart 结构"
            );
            assert!(
                source_texts.iter().any(|t| t.contains("sequenceDiagram")),
                "源码文件应保留 sequenceDiagram 结构"
            );
        }
        Err(e) => panic!("extract_doc 失败: {}", e),
    }
}

/// 独立画板节点（wiki board/whiteboard 节点）端到端：
/// Board 分支应导出预览图 + 独立源码文件（.mmd/.puml）。
/// 用真实画板 token 构造 board 类型节点树直接调用 wiki 导出，
/// 不依赖服务端恰好返回 board 类型的节点（实测画板文件常被识别为 docx）。
#[tokio::test]
async fn board_standalone_node_export() {
    const WB_TOKEN: &str = "D99dwaS0UhRdQ2btjj1cJtJTnJh";
    let settings = Settings::default();
    let stage = format!("{}/board_node", BASE);
    reset(&stage);

    let tree = WikiNode {
        node_token: "board-test-node".to_string(),
        title: "独立画板节点测试".to_string(),
        obj_type: WikiNodeType::Board,
        has_child: false,
        obj_token: Some(WB_TOKEN.to_string()),
        position: 0,
        depth: 0,
        children: vec![],
    };

    let result = extract_wiki_tree_controlled(tree, &stage, &settings, None, None, None)
        .await
        .expect("board 节点导出不应失败");

    println!("output_root = {}", result.output_root);
    println!(
        "[board_node] total={} success={} failed={} partial={}",
        result.total, result.success_count, result.failed_count, result.partial_count
    );
    print_tree(Path::new(&stage), 0);

    let images = collect_files(Path::new(&stage), &|p: &Path| {
        p.file_name()
            .map(|n| {
                let name = n.to_string_lossy();
                name.ends_with(".jpg") || name.ends_with(".png")
            })
            .unwrap_or(false)
    });
    println!("画板预览图: {} 张", images.len());
    assert_eq!(images.len(), 1, "board 节点应恰好导出 1 张预览图");
    for p in &images {
        let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        println!("  {} ({} bytes)", p.display(), size);
        assert!(
            size > 1024,
            "画板预览图过小: {} ({} bytes)",
            p.display(),
            size
        );
    }

    let sources = collect_files(Path::new(&stage), &|p: &Path| {
        p.file_name()
            .map(|n| {
                let name = n.to_string_lossy();
                name.ends_with(".mmd") || name.ends_with(".puml")
            })
            .unwrap_or(false)
    });
    println!("画板源码文件: {} 份", sources.len());
    assert_eq!(sources.len(), 1, "board 节点应恰好导出 1 份源码文件");
    for p in &sources {
        let text = std::fs::read_to_string(p).expect("源码文件应可读");
        println!("  {} => {}", p.display(), text.replace('\n', " / "));
        assert!(!text.trim().is_empty(), "源码文件为空: {}", p.display());
        assert!(
            text.contains("flowchart TD"),
            "源码文件应保留原始 mermaid 结构"
        );
    }

    assert!(
        result.failed_count == 0 && result.export_failures.is_empty(),
        "board 节点导出不应失败: {:?}",
        result.export_failures
    );
}

/// 画板文件实物挂到知识库后，按节点选中导出：
/// 实际画板文件在 wiki 里 obj_type=docx，导出走文档路径解析画板块，
/// 仍应产出预览图 + mmd 源码 + md 源码块。
#[tokio::test]
async fn board_wiki_node_export() {
    const WIKI_URL: &str = "https://qcny2iztd1p8.feishu.cn/wiki/LF3IwMPYkidVumkBka7cA6CPnab";
    const NODE_TOKEN: &str = "LF3IwMPYkidVumkBka7cA6CPnab";
    let settings = Settings::default();
    let stage = format!("{}/wiki_node", BASE);
    reset(&stage);

    let result = extract_wiki(WIKI_URL, &stage, &settings, Some(&[NODE_TOKEN.to_string()]))
        .await
        .expect("画板节点导出不应失败");

    println!("output_root = {}", result.output_root);
    println!(
        "[wiki_node] total={} success={} failed={} partial={}",
        result.total, result.success_count, result.failed_count, result.partial_count
    );
    print_tree(Path::new(&stage), 0);

    // 预览图
    let images = collect_files(Path::new(&stage), &|p: &Path| {
        p.file_name()
            .map(|n| {
                let name = n.to_string_lossy();
                (name.ends_with(".jpg") || name.ends_with(".png")) && name.contains("wb_tag_")
            })
            .unwrap_or(false)
    });
    println!("画板预览图: {} 张", images.len());
    assert_eq!(images.len(), 1, "画板文件节点应恰好导出 1 张预览图");
    let size = std::fs::metadata(&images[0]).map(|m| m.len()).unwrap_or(0);
    println!("  {} ({} bytes)", images[0].display(), size);
    assert!(
        size > 1024,
        "画板预览图过小: {} ({} bytes)",
        images[0].display(),
        size
    );

    // 源码文件
    let sources = collect_files(Path::new(&stage), &|p: &Path| {
        p.file_name()
            .map(|n| {
                let name = n.to_string_lossy();
                name.ends_with(".mmd") || name.ends_with(".puml")
            })
            .unwrap_or(false)
    });
    println!("画板源码文件: {} 份", sources.len());
    assert_eq!(sources.len(), 1, "画板文件节点应恰好导出 1 份源码文件");
    let text = std::fs::read_to_string(&sources[0]).expect("源码文件应可读");
    println!(
        "  {} => {}",
        sources[0].display(),
        text.replace('\n', " / ")
    );
    assert!(
        text.contains("flowchart TD"),
        "源码文件应保留原始 mermaid 结构"
    );

    // md 正文包含源码块与图片引用
    let md = collect_text_files(Path::new(&stage), "md");
    let hit = md
        .iter()
        .find(|(_, c)| c.contains("![画板]") && c.contains("```mermaid"))
        .cloned()
        .expect("md 正文应含画板图片引用与 mermaid 源码块");
    println!("含画板链接+源码的正文: {}", hit.0.display());

    assert!(
        result.failed_count == 0 && result.export_failures.is_empty(),
        "画板节点导出不应失败: {:?}",
        result.export_failures
    );
}

/// 把画板测试文档导出到临时目录，供归档到 docs/e2e-download-case/
#[tokio::test]
async fn export_board_case_to_archive() {
    let settings = Settings::default();
    let stage = format!("{}/case", BASE);
    reset(&stage);
    match extract::extract_doc_async(DOC_URL, &stage, &settings).await {
        Ok(r) => {
            println!(
                "[case] {}  图 {}/{} (失败 {})  字数 {}  错误: {:?}",
                r.filename,
                r.images_downloaded,
                r.image_count,
                r.images_failed,
                r.char_count,
                r.errors
            );
            println!("===== 目录树 stage=case =====");
            print_tree(Path::new(&stage), 0);
            assert!(
                r.errors.is_empty(),
                "画板测试文档导出不应有错误: {:?}",
                r.errors
            );
        }
        Err(e) => panic!("extract_doc 失败: {}", e),
    }
}

fn reset(dir: &str) {
    let p = Path::new(dir);
    if p.exists() {
        std::fs::remove_dir_all(p).expect("清理旧产物失败");
    }
    std::fs::create_dir_all(p).expect("创建输出目录失败");
}

fn collect_text_files(dir: &Path, ext: &str) -> Vec<(std::path::PathBuf, String)> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(collect_text_files(&p, ext));
            } else if p
                .extension()
                .map(|x| x.eq_ignore_ascii_case(ext))
                .unwrap_or(false)
            {
                if let Ok(c) = std::fs::read_to_string(&p) {
                    out.push((p.clone(), c));
                }
            }
        }
    }
    out
}

fn collect_files<F: Fn(&Path) -> bool>(dir: &Path, pred: &F) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(collect_files(&p, pred));
            } else if pred(&p) {
                out.push(p);
            }
        }
    }
    out
}

fn print_tree(dir: &Path, depth: usize) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().collect())
        .unwrap_or_default();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = path.is_dir();
        println!(
            "{}{} {}",
            "    ".repeat(depth),
            if is_dir { "DIR " } else { "FILE" },
            name
        );
        if is_dir {
            print_tree(&path, depth + 1);
        }
    }
}
