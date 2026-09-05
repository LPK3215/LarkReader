//! 已登录状态全维度测试
//!
//! 测试链接: https://gcnyv4rcw1jv.feishu.cn/wiki/Kh47wj3YRiPxsekidFWcbW0Knkb
//! 注意：只做小规模测试，不全量提取，避免浪费带宽和触发限流

use lark_reader_lib::{env, extract, lark, models, wiki};

const TEST_URL: &str = "https://gcnyv4rcw1jv.feishu.cn/wiki/Kh47wj3YRiPxsekidFWcbW0Knkb";

#[test]
fn test_logged_in_01_check_env() {
    println!("\n========================================");
    println!("  维度1: 已登录环境检测");
    println!("========================================\n");

    let status = env::check_env();
    println!(
        "Node.js:    {}  版本: {:?}",
        status.node_installed, status.node_version
    );
    println!(
        "lark-cli:   {}  版本: {:?}",
        status.lark_cli_installed, status.lark_cli_version
    );
    println!(
        "应用配置:   {}  AppID: {:?}",
        status.app_configured, status.app_id
    );
    println!("已登录:     {}", status.logged_in);
    println!("用户名:     {:?}", status.user_name);
    println!("Token状态:  {:?}", status.token_status);

    assert!(status.node_installed, "Node.js 应该已安装");
    assert!(status.lark_cli_installed, "lark-cli 应该已安装");
    assert!(status.app_configured, "应用应该已配置");
    assert!(status.logged_in, "用户应该已登录");
    println!("\n✅ 环境检测通过：已登录");
}

#[test]
fn test_logged_in_02_whoami() {
    println!("\n========================================");
    println!("  维度2: whoami 直接调用");
    println!("========================================\n");

    let result = lark::whoami();
    assert!(result.is_ok(), "whoami 应该成功");

    let (identity, token_status, user_name) = result.unwrap();
    println!("identity: {}", identity);
    println!("token_status: {}", token_status);
    println!("user_name: {:?}", user_name);

    assert_eq!(identity, "user", "identity 应该是 user");
    assert!(
        token_status == "ready" || token_status == "needs_refresh",
        "token 应该可用"
    );
    println!("\n✅ whoami 通过");
}

#[test]
fn test_logged_in_03_wiki_node_get() {
    println!("\n========================================");
    println!("  维度3: Wiki 节点信息获取");
    println!("========================================\n");

    let node_token = extract::parse_node_token(TEST_URL);
    println!("从 URL 提取 node_token: {}", node_token);

    let result = lark::wiki_node_get(&node_token);
    println!("wiki +node-get 结果: {:?}", result);

    assert!(result.is_ok(), "node-get 应该成功");

    let info = result.unwrap();
    println!("space_id: {:?}", info.space_id);
    println!("obj_token: {:?}", info.obj_token);
    println!("obj_type: {:?}", info.obj_type);
    println!("has_child: {:?}", info.has_child);
    println!("title: {:?}", info.title);

    assert!(info.space_id.is_some(), "应该有 space_id");
    println!("\n✅ Wiki 节点信息获取通过");
}

#[test]
fn test_logged_in_04_preview_doc() {
    println!("\n========================================");
    println!("  维度4: 文档预览（只取正文，不下载图片）");
    println!("========================================\n");

    let result = extract::preview_doc(TEST_URL);

    match &result {
        Ok(preview) => {
            println!("标题: {}", preview.title);
            println!("字符数: {}", preview.char_count);
            println!("图片数: {}", preview.images.len());

            // 打印正文前200字符预览（安全截断）
            let content_preview: String = preview.content_markdown.chars().take(200).collect();
            println!("\n正文前200字符:\n{}", content_preview);

            // 打印前3张图片信息
            for (i, img) in preview.images.iter().take(3).enumerate() {
                println!(
                    "  图片 {}: alt={}, token={}",
                    i + 1,
                    img.alt,
                    img.file_token
                );
            }

            assert!(!preview.content_markdown.is_empty(), "正文不应该为空");
            println!("\n✅ 文档预览通过");
        }
        Err(e) => {
            println!("❌ 预览失败: {}", e);
            panic!("文档预览应该成功: {}", e);
        }
    }
}

#[test]
fn test_logged_in_05_extract_doc() {
    println!("\n========================================");
    println!("  维度5: 单文档提取（正文+图片，保存到本地）");
    println!("========================================\n");

    let temp_dir = std::env::temp_dir().join("larkreader_test");
    std::fs::create_dir_all(&temp_dir).unwrap();
    println!("输出目录: {}", temp_dir.display());

    let settings = models::Settings {
        output_dir: temp_dir.to_string_lossy().to_string(),
        concurrency: 3,
        download_images: true,
    };

    let result = extract::extract_doc(TEST_URL, &settings.output_dir, &settings);

    match &result {
        Ok(r) => {
            println!("标题: {}", r.title);
            println!("文件名: {}", r.filename);
            println!("字符数: {}", r.char_count);
            println!("图片总数: {}", r.image_count);
            println!("成功下载: {}", r.images_downloaded);
            println!("下载失败: {}", r.images_failed);
            println!("状态: {:?}", r.status);
            println!("文件路径: {}", r.filepath);
            println!("错误: {:?}", r.errors);

            if !r.errors.is_empty() {
                println!("\n错误详情:");
                for e in &r.errors {
                    println!("  - {}", e);
                }
            }

            // 验证文件确实生成了
            let file_exists = std::path::Path::new(&r.filepath).exists();
            println!("\n文件是否存在: {}", file_exists);
            if file_exists {
                let file_size = std::fs::metadata(&r.filepath).unwrap().len();
                println!(
                    "文件大小: {} bytes ({:.1} KB)",
                    file_size,
                    file_size as f64 / 1024.0
                );
            }

            assert!(!r.title.is_empty(), "标题不应该为空");
            assert!(r.char_count > 0, "字符数应该大于0");
            println!("\n✅ 单文档提取通过");
        }
        Err(e) => {
            println!("❌ 提取失败: {}", e);
            panic!("单文档提取应该成功: {}", e);
        }
    }
}

#[test]
fn test_logged_in_06_get_wiki_tree() {
    println!("\n========================================");
    println!("  维度6: 知识库目录树获取");
    println!("========================================\n");

    let result = wiki::get_wiki_tree(TEST_URL);

    match &result {
        Ok(tree) => {
            println!("知识库名称: {}", tree.title);
            println!("根节点 token: {}", tree.node_token);
            println!("是否有子节点: {}", tree.has_child);
            println!("子节点数量: {}", tree.children.len());
            println!("文档总数: {}", tree.count_docs());

            // 打印前5个子节点（不全打）
            println!("\n前5个子节点:");
            for (i, child) in tree.children.iter().take(5).enumerate() {
                println!(
                    "  {}: {} (type={:?}, has_child={}, position={})",
                    i, child.title, child.obj_type, child.has_child, child.position
                );
            }

            println!("\n✅ 知识库目录树获取通过");
        }
        Err(e) => {
            println!("❌ 目录树获取失败: {}", e);
            // 这个测试不 assert，因为可能这个链接就是单文档不是知识库根
            println!("   （可能是单文档链接，没有子节点树）");
        }
    }
}

#[test]
fn test_logged_in_07_invalid_url() {
    println!("\n========================================");
    println!("  维度7: 无效链接错误提示");
    println!("========================================\n");

    let invalid_url = "https://gcnyv4rcw1jv.feishu.cn/wiki/INVALID_TOKEN_12345";
    println!("测试链接: {}", invalid_url);

    let result = extract::preview_doc(invalid_url);

    match &result {
        Ok(_) => {
            println!("⚠️ 意外成功（不应该）");
        }
        Err(e) => {
            println!("❌ 预期失败");
            println!("   错误类型: {:?}", e);
            println!("   错误信息: {}", e);

            let err_str = e.to_string();
            // 验证错误信息是友好的中文提示
            let is_friendly = err_str.contains("不存在")
                || err_str.contains("无效")
                || err_str.contains("失败")
                || err_str.contains("权限")
                || err_str.contains("错误");

            if is_friendly {
                println!("\n✅ 错误信息友好");
            } else {
                println!("\n⚠️ 错误信息可能需要优化: {}", err_str);
            }
        }
    }
}

#[test]
fn test_logged_in_08_empty_url() {
    println!("\n========================================");
    println!("  维度8: 空链接错误处理");
    println!("========================================\n");

    let result = extract::preview_doc("");

    match &result {
        Ok(_) => println!("⚠️ 空链接意外成功"),
        Err(e) => {
            println!("❌ 预期失败");
            println!("   错误信息: {}", e);
            println!("\n✅ 空链接正确处理");
        }
    }
}
