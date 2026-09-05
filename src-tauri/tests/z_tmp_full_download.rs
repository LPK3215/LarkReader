//! E2E 真实下载回归：调用产品 extract_wiki 全库导出逻辑，把 LarkReader-E2E-测试库
//! 内容下载到项目内临时目录 e2e_download_tmp/（见 .gitignore），验证真实导出结构与完整性。
//!
//! 前提：本机已登录 lark-cli（user 身份）且有该知识库访问权。产物为工作目录，
//! 每次运行前请先清空 e2e_download_tmp/，避免 `01_文本排版 (2)/(3)` 式重复目录累积。
//!
//! - download_everything：7 个顶层根节点全量导出（对应 docs/e2e-download-case 的前 7 根）
//! - download_attachments：06_附件下载 容器，断言 18 个 file 附件全部真实落盘且扩展名保留
//!
//! 相关说明见 docs/e2e-download-case/README.md 与 docs/BACKEND.md 第 6 节。

use lark_reader_lib::{models::Settings, wiki};
use std::path::Path;

/// 输出到项目根目录下的 e2e_download_tmp
const OUT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../e2e_download_tmp");

/// E2E 测试库空间根直属的全部顶层节点（node-list space-id 无 parent 的 7 个）
const ROOTS: &[&str] = &[
    "https://feishu.cn/wiki/LuQ7wEqgmiqITJkL49zcjyA0nif", // 01_文本排版（单文档页）
    "https://feishu.cn/wiki/EAMOwgdxZiuJcGk7SggcXFGnnXf", // 01_文本排版（3 个子文档）
    "https://feishu.cn/wiki/GALuwGcesiqpOXkFIKYccYtbnZo", // 02_媒体（图片/附件等）
    "https://feishu.cn/wiki/KpGawWRGtif1PGkdAbscQBGZntd", // 03_表格数据库（Sheet/Bitable）
    "https://feishu.cn/wiki/LhydwRkCdil1oNkn1eHccMrhnDf", // 04_边界情况
    "https://feishu.cn/wiki/WggMwupWaiRueskwSTtc0J00nId", // 05_排序验证
    "https://feishu.cn/wiki/CnMowYMTOiHj3pkWa66cHqfRnne", // 特殊字符标题文档（非法字符文件名）
];

#[test]
fn download_everything() {
    let settings = Settings::default();
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime 失败");

    for (i, url) in ROOTS.iter().enumerate() {
        println!("\n========== 根节点 {}/{}  {}", i + 1, ROOTS.len(), url);
        match rt.block_on(wiki::extract_wiki(url, OUT, &settings, None)) {
            Ok(r) => {
                println!(
                    "知识库目录: {}  |  成功 {} / 部分 {} / 失败 {} / 跳过 {} / 总 {}",
                    r.output_root,
                    r.success_count,
                    r.partial_count,
                    r.failed_count,
                    r.skipped_count,
                    r.total
                );
                for it in &r.results {
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
                for ex in &r.exports {
                    println!("  [特殊] {} -> {:?}", ex.title, ex.paths);
                }
                for f in &r.failures {
                    println!("  [doc失败] {} : {}", f.title, f.error);
                }
                for f in &r.export_failures {
                    println!("  [特殊失败] {} : {}", f.title, f.error);
                }
                for s in &r.skipped {
                    println!("  [跳过] {} : {}", s.title, s.reason);
                }
            }
            Err(e) => println!("  ❌ extract_wiki 失败: {}", e),
        }
    }

    println!("\n========== 项目内输出目录结构 e2e_download_tmp/ ==========");
    print_tree(Path::new(OUT), 0);

    assert!(Path::new(OUT).exists(), "输出目录应存在");
}

/// 06_附件下载 页面（挂载 18 个多格式附件），验证 file 节点附件下载管道
#[test]
fn download_attachments() {
    const URL: &str = "https://qcny2iztd1p8.feishu.cn/wiki/Iwhuw7jDbiwvm1kodW6cCpZVnsb";
    let settings = Settings::default();
    let rt = tokio::runtime::Runtime::new().expect("创建 tokio runtime 失败");

    println!("========== 附件容器节点下载 ==========");
    match rt.block_on(wiki::extract_wiki(URL, OUT, &settings, None)) {
        Ok(r) => {
            println!(
                "知识库目录: {}  |  成功 {} / 部分 {} / 失败 {} / 跳过 {} / 总 {}",
                r.output_root,
                r.success_count,
                r.partial_count,
                r.failed_count,
                r.skipped_count,
                r.total
            );
            for ex in &r.exports {
                let size = ex
                    .paths
                    .first()
                    .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
                    .unwrap_or(0);
                println!(
                    "  [附件] {} ({}) -> {}",
                    ex.title,
                    size,
                    ex.paths.first().unwrap_or(&String::new())
                );
            }
            for f in &r.export_failures {
                println!("  [特殊失败] {} : {}", f.title, f.error);
            }
            for s in &r.skipped {
                println!("  [跳过] {} : {}", s.title, s.reason);
            }

            assert_eq!(r.failed_count, 0, "附件下载不应有 doc 失败");
            assert_eq!(r.skipped_count, 0, "file 附件不应再被跳过");
            assert!(r.export_failures.is_empty(), "不应有特殊导出失败");
            assert_eq!(r.exports.len(), 18, "应恰好导出 18 个附件");

            for ex in &r.exports {
                assert_eq!(ex.paths.len(), 1, "附件应恰好一个落盘路径: {}", ex.title);
                let p = Path::new(&ex.paths[0]);
                assert!(p.exists(), "附件应真实落盘: {} ({})", ex.title, p.display());
                let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                assert!(size > 0, "附件不应为空: {} ({} bytes)", ex.title, size);
                // 扩展名应保留在落盘文件名中（drive +preview source_file 原样落盘）
                if let Some((_, ext)) = ex.title.rsplit_once('.') {
                    let fname = p.file_name().unwrap().to_string_lossy().into_owned();
                    assert!(
                        fname.ends_with(&format!(".{}", ext)),
                        "落盘文件名应保留扩展名 .{}: {}",
                        ext,
                        fname
                    );
                }
            }
        }
        Err(e) => panic!("extract_wiki 失败: {}", e),
    }

    println!("\n========== 输出目录结构 ==========");
    print_tree(Path::new(OUT), 0);
}

/// 递归打印目录树
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
            if is_dir { "DIR" } else { "FILE" },
            name
        );
        if is_dir {
            print_tree(&path, depth + 1);
        }
    }
}
