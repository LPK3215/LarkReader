//! 后端集成测试 — 验证 lark-cli 调用链
//!
//! 运行方式: cargo test --test integration -- --nocapture

use lark_reader_lib::{env, extract, lark, markdown, models};

#[test]
fn test_01_check_env_matches_identity() {
    let status = env::check_env();
    println!("=== 环境检测结果 ===");
    println!(
        "Node.js: {} ({:?})",
        status.node_installed, status.node_version
    );
    println!(
        "lark-cli: {} ({:?})",
        status.lark_cli_installed, status.lark_cli_version
    );
    println!("应用配置: {} ({:?})", status.app_configured, status.app_id);
    println!("已登录: {}", status.logged_in);
    println!("用户名: {:?}", status.user_name);
    println!("Token状态: {:?}", status.token_status);

    // lark-cli 应该已安装
    assert!(status.node_installed, "Node.js 应该已安装");
    assert!(status.lark_cli_installed, "lark-cli 应该已安装");
    assert!(
        status.app_configured,
        "飞书应用应该已配置（config show 应该能返回 appId）"
    );

    let identity = lark::whoami().expect("whoami 应该成功");
    let expected = identity.0 == "user" && (identity.1 == "ready" || identity.1 == "needs_refresh");
    assert_eq!(status.logged_in, expected, "环境检测应与真实身份一致");
}

#[test]
fn test_02_lark_cli_version() {
    let version = lark::lark_cli_version();
    println!("=== lark-cli 版本 ===");
    println!("{:?}", version);
    assert!(version.is_ok(), "获取版本号应该成功");
    println!("版本: {}", version.unwrap());
}

#[test]
fn test_03_config_show() {
    let config = lark::config_show();
    println!("=== config show ===");
    println!("{:?}", config);
    assert!(config.is_ok(), "config show 应该成功");

    let config = config.unwrap();
    assert!(config.is_some(), "应用配置应该存在");
    let (app_id, brand) = config.unwrap();
    println!("appId: {}", app_id);
    println!("brand: {}", brand);
    assert!(!app_id.is_empty());
}

#[test]
fn test_04_whoami() {
    let result = lark::whoami();
    println!("=== whoami ===");
    println!("{:?}", result);
    assert!(result.is_ok());

    let (identity, token_status, user_name) = result.unwrap();
    println!("identity: {}", identity);
    println!("token_status: {}", token_status);
    println!("user_name: {:?}", user_name);
}

#[test]
fn test_05_markdown_extract_images() {
    let content = r#"# 测试文档

![图片1](https://feishu.cn/file/boxcnABC123)

正文内容

![图片2](https://internal-api-drive-stream.feishu.cn/suite/api/v1/file/boxcnDEF456?param=1)
"#;
    let images = markdown::extract_images(content);
    println!("=== Markdown 图片提取 ===");
    println!("找到 {} 张图片", images.len());
    for (i, img) in images.iter().enumerate() {
        println!("  {}: alt={}, token={}", i + 1, img.alt, img.file_token);
    }
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].file_token, "boxcnABC123");
    assert_eq!(images[1].file_token, "boxcnDEF456");
}

#[test]
fn test_06_safe_filename() {
    println!("=== 文件名安全化 ===");
    assert_eq!(markdown::safe_filename("hello/world"), "hello_world");
    assert_eq!(markdown::safe_filename("test:*?file"), "test___file");
    assert_eq!(markdown::safe_filename("正常标题"), "正常标题");
    println!("全部通过");
}

#[test]
fn test_07_prefixed_filename() {
    println!("=== 位置前缀文件名 ===");
    assert_eq!(markdown::prefixed_filename(0, "第一章"), "00_第一章");
    assert_eq!(markdown::prefixed_filename(5, "技术文档"), "05_技术文档");
    assert_eq!(markdown::prefixed_filename(12, "test"), "12_test");
    println!("全部通过");
}

#[test]
fn test_08_parse_node_token() {
    println!("=== URL 解析 ===");
    assert_eq!(
        extract::parse_node_token(
            "https://gcnyv4rcw1jv.feishu.cn/wiki/QJFEw6cH0iSry4kRUcMcDttfn4e"
        ),
        "QJFEw6cH0iSry4kRUcMcDttfn4e"
    );
    assert_eq!(
        extract::parse_node_token("QJFEw6cH0iSry4kRUcMcDttfn4e"),
        "QJFEw6cH0iSry4kRUcMcDttfn4e"
    );
    println!("全部通过");
}

#[test]
fn test_09_build_wiki_url() {
    println!("=== Wiki URL 构造 ===");
    let url = extract::build_wiki_url("ABC123");
    assert_eq!(url, "https://feishu.cn/wiki/ABC123");

    let url2 = extract::build_wiki_url("https://xxx.feishu.cn/wiki/ABC123");
    assert_eq!(url2, "https://xxx.feishu.cn/wiki/ABC123");
    println!("全部通过");
}

#[test]
fn test_10_settings_default() {
    let settings = models::Settings::default();
    println!("=== 默认设置 ===");
    println!("output_dir: {}", settings.output_dir);
    println!("concurrency: {}", settings.concurrency);
    println!("download_images: {}", settings.download_images);

    assert!(!settings.output_dir.is_empty());
    assert_eq!(settings.concurrency, 5);
    assert!(settings.download_images);
}
