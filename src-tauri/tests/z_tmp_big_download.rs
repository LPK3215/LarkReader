//! 真实课程知识库【百战程序员】大库下载回归（需登录 lark-cli 且对该库有访问权）
//!
//! - download_5_single_docs：只选 5 篇单文档（不同深度），观察父目录链是否保留
//! - download_multi_selection：多选（含“选中父目录整节点自动带全部后代”）
//! - download_full_knowledge：不选 = 整棵全量（Doc 正文 + file 附件 zip）
//! - download_zip_attachment：只选 zip 附件节点（obj_type=file），验证 drive +preview 下载
//!
//! 每个阶段输出到 `e2e_download_tmp_big/<stage>/`（见 .gitignore），运行前清空对应旧产物。

use lark_reader_lib::{models::Settings, wiki};
use std::path::Path;

const ROOT_URL: &str = "https://gcnyv4rcw1jv.feishu.cn/wiki/Kh47wj3YRiPxsekidFWcbW0Knkb";
const BASE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../e2e_download_tmp_big");

// ---- 阶段 1：5 篇单文档，覆盖由浅到深的 4 种位置 ----
const DOC_ROOT_LEAF: &str = "QJFEw6cH0iSry4kRUcMcDttfn4e"; // 从0开始学习agent（根下直接叶子）
const DOC_L1_1: &str = "VYVnwD6iKinzElkIn6jcMQjNn3g"; // 智能体工具/Claude Code 使用（一层父）
const DOC_L1_2: &str = "PreewHW0Aijbgukm4wecvlXJnUg"; // 代码生成Agent/HumanEval（一层父）
const DOC_L1_3: &str = "VJOSwG6n8iKzpzkDqikcwV5unbe"; // 大模型微调完全指南/DeepSeek微调（一层父，独苗）
const DOC_L2: &str = "E4GKwYd1ki2QoVkhPWfcDoLKnSg"; // 智能体工具/OpenClaw进阶/工具（二层父链）

// ---- 阶段 2：多选（混合 父节点整选 + 散叶）----
const SEL_PARENT_11PROJ: &str = "TIWBwjYeaiRsekkVUeic0xzbndf"; // 11个企业级Agent实战项目（父，整棵 12 节点）
const SEL_PARENT_MEM: &str = "ApmewpaJWiiexVk2rtHcghSUnqd"; // Memory与状态管理（父，2 节点）
const SEL_ROOT_LEAF_1: &str = "EKc8wYUawiRjKgkn1LYc6DKGn4b"; // Agent技术（根下叶）
const SEL_ROOT_LEAF_2: &str = "SVybwhyKgiCYvWkTH6ZcrjpOnbf"; // MCP协议（根下叶）
const SEL_L1_LEAF: &str = "Wstaw7A8dikIsvkONI8cTAJfnVh"; // Agent安全与护栏/NeMo Guardrails（1层叶）
const SEL_L2_LEAF: &str = "Ob3Iw3xCCijNZ5kAeUvcHqncnkd"; // 智能体工具/Hermes-Agent/Hermes Agent 使用（2层叶）
const SEL_PROJ_LEAF: &str = "Nin9wJXwci4clUk3T0qcGV18nRe"; // 19个项目/Agent项目-PaperAI（散叶，深2层）
const SEL_PROJ_LEAF19: &str = "L1TbwYp7XieKlckWScDcuEDTnwg"; // 19个项目/电商智能客服（散叶，深2层）

fn main_stats(r: &lark_reader_lib::models::WikiExtractResult, stage: &str) {
    println!(
        "[{}] 成功 {} / 部分 {} / 失败 {} / 跳过 {} / 总 {}",
        stage, r.success_count, r.partial_count, r.failed_count, r.skipped_count, r.total
    );
    println!("[{}] 输出根: {}", stage, r.output_root);
}

fn print_doc_item(it: &lark_reader_lib::models::ExtractResult) {
    println!(
        "  [doc] {}  图 {}/{} (失败 {})  字数 {}  错误: {:?}",
        it.filename,
        it.images_downloaded,
        it.image_count,
        it.images_failed,
        it.char_count,
        it.errors
    );
}

/// 递归统计输出目录下的 .md 文件数（即实际落盘正文数）
fn count_md(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += count_md(&p);
            } else if p
                .extension()
                .map(|x| x.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
            {
                n += 1;
            }
        }
    }
    n
}

#[tokio::test]
async fn download_5_single_docs() {
    let settings = Settings::default();
    let stage = format!("{}/single5", BASE);
    reset(&stage);
    let selected: Vec<String> = vec![
        DOC_ROOT_LEAF.into(),
        DOC_L1_1.into(),
        DOC_L1_2.into(),
        DOC_L1_3.into(),
        DOC_L2.into(),
    ];
    match wiki::extract_wiki(ROOT_URL, &stage, &settings, Some(&selected)).await {
        Ok(r) => {
            main_stats(&r, "single5");
            for it in &r.results {
                print_doc_item(it);
            }
            for f in &r.failures {
                println!("  [doc失败] {} : {}", f.title, f.error);
            }
            for s in &r.skipped {
                println!("  [跳过] {} : {}", s.title, s.reason);
            }
            println!("===== 目录树 stage=single5 =====");
            print_full_tree(Path::new(&stage), 0);
            assert_eq!(r.failed_count, 0, "5 篇单文档不应有失败");
            let md = count_md(Path::new(&stage));
            println!("落盘正文 .md 总数 = {}（期望 5）", md);
            assert_eq!(md, 5, "只选 5 篇单文档，不应额外下载父文档正文");
        }
        Err(e) => panic!("extract_wiki 失败: {}", e),
    }
}

#[tokio::test]
async fn download_multi_selection() {
    let settings = Settings::default();
    let stage = format!("{}/multi", BASE);
    reset(&stage);
    let selected: Vec<String> = vec![
        SEL_PARENT_11PROJ.into(),
        SEL_PARENT_MEM.into(),
        SEL_ROOT_LEAF_1.into(),
        SEL_ROOT_LEAF_2.into(),
        SEL_L1_LEAF.into(),
        SEL_L2_LEAF.into(),
        SEL_PROJ_LEAF.into(),
        SEL_PROJ_LEAF19.into(),
    ];
    match wiki::extract_wiki(ROOT_URL, &stage, &settings, Some(&selected)).await {
        Ok(r) => {
            main_stats(&r, "multi");
            for it in &r.results {
                print_doc_item(it);
            }
            for f in &r.failures {
                println!("  [doc失败] {} : {}", f.title, f.error);
            }
            for s in &r.skipped {
                println!("  [跳过] {} : {}", s.title, s.reason);
            }
            println!("===== 目录树 stage=multi =====");
            print_full_tree(Path::new(&stage), 0);
            assert_eq!(r.failed_count, 0, "多选不应有失败");
            let md = count_md(Path::new(&stage));
            println!(
                "落盘正文 .md 总数 = {}（期望 20 = 11项目父12 + Memory父2 + 散叶6）",
                md
            );
        }
        Err(e) => panic!("extract_wiki 失败: {}", e),
    }
}

#[tokio::test]
async fn download_full_knowledge() {
    let settings = Settings::default();
    let stage = format!("{}/full", BASE);
    reset(&stage);
    match wiki::extract_wiki(ROOT_URL, &stage, &settings, None).await {
        Ok(r) => {
            main_stats(&r, "full");
            for f in &r.failures {
                println!("  [doc失败] {} : {}", f.title, f.error);
            }
            for f in &r.export_failures {
                println!("  [特殊失败] {} : {}", f.title, f.error);
            }
            for s in &r.skipped {
                println!("  [跳过] {} : {}", s.title, s.reason);
            }
            let md = count_md(Path::new(&stage));
            println!(
                "落盘正文 .md 总数 = {}（期望 ≈ 全量 Doc 数；zip 附件经 file 通道落盘，不跳过了）",
                md
            );
            println!("===== 目录树 stage=full（目录层）=====");
            print_dirs_only(Path::new(&stage), 0);
        }
        Err(e) => panic!("extract_wiki 失败: {}", e),
    }
}

// ---- 阶段 4：单文件附件（zip，obj_type=file）----
const ZIP_ATTACH: &str = "JuIdwJkBoiJ8rLkkNZHciOfOn2c"; // Custom-Image-API-Skill/codex-image能力….zip

#[tokio::test]
async fn download_zip_attachment() {
    let settings = Settings::default();
    let stage = format!("{}/zip", BASE);
    reset(&stage);
    let selected = vec![ZIP_ATTACH.to_string()];
    match wiki::extract_wiki(ROOT_URL, &stage, &settings, Some(&selected)).await {
        Ok(r) => {
            main_stats(&r, "zip");
            for e in &r.exports {
                println!("  [文件] {} : {:?}", e.title, e.paths);
            }
            for s in &r.skipped {
                println!("  [跳过] {} : {}", s.title, s.reason);
            }
            for f in &r.export_failures {
                println!("  [文件失败] {} : {}", f.title, f.error);
            }
            assert_eq!(r.failed_count, 0, "附件下载不应有失败");
            assert_eq!(r.skipped_count, 0, "file 节点不应再被跳过");
            assert_eq!(r.exports.len(), 1, "应恰好导出 1 个附件");
            let file = Path::new(&r.exports[0].paths[0]);
            assert!(file.exists(), "zip 应真实落盘: {}", file.display());
            let size = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
            assert!(size > 1000, "zip 大小应大于 1KB，实际 {}", size);
            println!("附件落盘: {} ({} bytes)", file.display(), size);
            println!("===== 目录树 stage=zip =====");
            print_full_tree(Path::new(&stage), 0);
        }
        Err(e) => panic!("extract_wiki 失败: {}", e),
    }
}

fn reset(dir: &str) {
    let p = Path::new(dir);
    if p.exists() {
        std::fs::remove_dir_all(p).expect("清理旧产物失败");
    }
    std::fs::create_dir_all(p).expect("创建输出目录失败");
}

/// 完整目录树（文件+目录），树较小时用
fn print_full_tree(dir: &Path, depth: usize) {
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
            print_full_tree(&path, depth + 1);
        }
    }
}

/// 只打印目录层（含每个目录的文件计数），全量树太大时用
fn print_dirs_only(dir: &Path, depth: usize) {
    let mut dirs: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).collect())
        .unwrap_or_default();
    dirs.sort_by_key(|e| e.file_name());
    for entry in dirs {
        let path = entry.path();
        println!(
            "{}{} （{} 个文件）",
            "    ".repeat(depth),
            entry.file_name().to_string_lossy().into_owned(),
            count_md(&path)
        );
        print_dirs_only(&path, depth + 1);
    }
}
