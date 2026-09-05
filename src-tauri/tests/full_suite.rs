//! 全流程综合测试矩阵
//!
//! 覆盖：
//! A. 纯函数测试（不需要 lark-cli，不依赖登录状态）
//! B. 已登录正常流程（文档预览、提取、图片下载、知识库树）
//! C. 边界情况（空链接、无效链接、纯 token、不同 URL 格式）
//! D. 错误处理（错误信息是否友好、是否 panic）
//! E. 重复执行稳定性（同一操作多次执行无异常）

use lark_reader_lib::{env, extract, lark, markdown, models, wiki};

const TEST_URL: &str = "https://gcnyv4rcw1jv.feishu.cn/wiki/Kh47wj3YRiPxsekidFWcbW0Knkb";
const SUB_DOC_URL: &str = "https://gcnyv4rcw1jv.feishu.cn/wiki/QJFEw6cH0iSry4kRUcMcDttfn4e";

// ============================================================================
// A. 纯函数测试（不需要网络，不需要登录）
// ============================================================================

#[test]
fn test_a01_parse_node_token_various_formats() {
    println!("\n=== A01: URL 解析各种格式 ===");

    let cases = vec![
        (
            "https://gcnyv4rcw1jv.feishu.cn/wiki/Kh47wj3YRiPxsekidFWcbW0Knkb",
            "Kh47wj3YRiPxsekidFWcbW0Knkb",
        ),
        ("https://feishu.cn/wiki/ABC123", "ABC123"),
        (
            "https://internal-api.feishu.cn/wiki/XYZ789?param=1",
            "XYZ789",
        ),
        ("Kh47wj3YRiPxsekidFWcbW0Knkb", "Kh47wj3YRiPxsekidFWcbW0Knkb"),
        ("ABC123", "ABC123"),
        ("https://gcnyv4rcw1jv.feishu.cn/wiki/Token/", "Token"),
    ];

    for (input, expected) in cases {
        let result = extract::parse_node_token(input);
        assert_eq!(
            result, expected,
            "parse_node_token(\"{}\") = \"{}\", expected \"{}\"",
            input, result, expected
        );
        println!("  ✅ {} → {}", input, result);
    }
}

#[test]
fn test_a02_build_wiki_url() {
    println!("\n=== A02: Wiki URL 构造 ===");

    assert_eq!(
        extract::build_wiki_url("ABC123"),
        "https://feishu.cn/wiki/ABC123"
    );
    assert_eq!(
        extract::build_wiki_url("https://xxx.feishu.cn/wiki/ABC123"),
        "https://xxx.feishu.cn/wiki/ABC123"
    );
    assert_eq!(
        extract::build_wiki_url("https://feishu.cn/wiki/XYZ"),
        "https://feishu.cn/wiki/XYZ"
    );
    println!("  ✅ 所有 URL 构造测试通过");
}

#[test]
fn test_a03_safe_filename_chinese() {
    println!("\n=== A03: 中文文件名安全化（不 panic）===");

    // 超长中文字符串不应该 panic
    let long_name = "中".repeat(200);
    let result = markdown::safe_filename(&long_name);
    assert!(result.chars().count() <= 100, "应该截断到 100 字符");
    println!(
        "  ✅ 超长中文字符串安全截断: {} 字符",
        result.chars().count()
    );

    // 特殊字符替换
    assert_eq!(markdown::safe_filename("测试/文件:名*"), "测试_文件_名_");
    println!("  ✅ 特殊字符替换正确");

    // 混合
    let mixed = "第1章\\Agent/:*?\"<>|简介";
    let result = markdown::safe_filename(mixed);
    assert!(!result.contains('/') && !result.contains('\\') && !result.contains(':'));
    println!("  ✅ 混合字符安全化: {}", result);
}

#[test]
fn test_a04_extract_images_from_markdown() {
    println!("\n=== A04: Markdown 图片提取 ===");

    let content = r#"
# 标题

![图片A](https://feishu.cn/file/tokenAAA)
正文文字
![图片B描述](https://internal-api-drive-stream.feishu.cn/suite/api/v1/file/tokenBBB?param=1)
![空描述]()
![带特殊字符的描述](https://feishu.cn/file/tokenCCC?x=1&y=2)
"#;

    let images = markdown::extract_images(content);
    assert_eq!(images.len(), 3, "应该提取 3 张图片（跳过空 URL 的）");
    assert_eq!(images[0].file_token, "tokenAAA");
    assert_eq!(images[1].file_token, "tokenBBB");
    assert_eq!(images[2].file_token, "tokenCCC");
    println!("  ✅ 提取到 {} 张图片", images.len());
    for (i, img) in images.iter().enumerate() {
        println!("    图片 {}: token={}", i + 1, img.file_token);
    }
}

#[test]
fn test_a05_replace_image_urls() {
    println!("\n=== A05: 图片 URL 替换 ===");

    let content = "![img](https://feishu.cn/file/tokenA)\n![img2](https://feishu.cn/file/tokenB)";
    let replaced = markdown::replace_image_urls(
        content,
        &[
            (
                "https://feishu.cn/file/tokenA".to_string(),
                "images/img_01.png".to_string(),
            ),
            (
                "https://feishu.cn/file/tokenB".to_string(),
                "images/img_02.png".to_string(),
            ),
        ],
    );
    assert!(replaced.contains("images/img_01.png"));
    assert!(replaced.contains("images/img_02.png"));
    assert!(!replaced.contains("https://feishu.cn/file/tokenA"));
    assert!(!replaced.contains("https://feishu.cn/file/tokenB"));
    println!("  ✅ URL 替换正确");
}

// ============================================================================
// B. 已登录正常流程
// ============================================================================

#[test]
fn test_b01_env_check_logged_in() {
    println!("\n=== B01: 环境检测（已登录）===");

    let status = env::check_env();
    println!(
        "  Node.js: {} {:?}",
        status.node_installed, status.node_version
    );
    println!(
        "  lark-cli: {} {:?}",
        status.lark_cli_installed, status.lark_cli_version
    );
    println!(
        "  已配置: {} AppID={:?}",
        status.app_configured, status.app_id
    );
    println!(
        "  已登录: {} Token={:?}",
        status.logged_in, status.token_status
    );

    assert!(status.node_installed, "Node.js 必须已安装");
    assert!(status.lark_cli_installed, "lark-cli 必须已安装");
    assert!(status.app_configured, "飞书应用必须已配置");
    assert!(status.logged_in, "用户必须已登录");
    println!("  ✅ 环境检测通过");
}

#[test]
fn test_b02_whoami() {
    println!("\n=== B02: whoami ===");

    let (identity, token_status, user_name) = lark::whoami().expect("whoami 失败");
    println!(
        "  identity={}, token={}, user={:?}",
        identity, token_status, user_name
    );

    assert_eq!(identity, "user", "identity 必须是 user");
    assert!(token_status == "ready" || token_status == "needs_refresh");
    println!("  ✅ whoami 通过");
}

#[test]
fn test_b03_config_show() {
    println!("\n=== B03: config show ===");

    let config = lark::config_show().expect("config show 失败");
    assert!(config.is_some(), "应用必须已配置");

    let (app_id, brand) = config.unwrap();
    println!("  AppID={}, brand={}", app_id, brand);
    assert!(!app_id.is_empty(), "AppID 不为空");
    println!("  ✅ config show 通过");
}

#[test]
fn test_b04_wiki_node_get() {
    println!("\n=== B04: wiki node-get ===");

    let node_token = extract::parse_node_token(TEST_URL);
    let info = lark::wiki_node_get(&node_token).expect("node-get 失败");

    println!("  title={:?}", info.title);
    println!("  space_id={:?}", info.space_id);
    println!("  obj_type={:?}", info.obj_type);
    println!("  has_child={:?}", info.has_child);

    assert!(info.space_id.is_some(), "必须有 space_id");
    assert!(info.title.is_some(), "必须有 title");
    println!("  ✅ wiki node-get 通过");
}

#[test]
fn test_b05_preview_doc_with_real_title() {
    println!("\n=== B05: 文档预览（含真实标题）===");

    let preview = extract::preview_doc(TEST_URL).expect("preview 失败");
    println!("  标题: {}", preview.title);
    println!("  字符数: {}", preview.char_count);
    println!("  图片数: {}", preview.images.len());

    // 标题应该是文档真实标题而不是 URL token
    assert!(!preview.title.is_empty(), "标题不为空");
    // 如果获取到了真实标题，应该包含中文
    if !preview.title.chars().all(char::is_alphanumeric) {
        println!("  ✅ 获取到了文档真实标题: {}", preview.title);
    } else {
        println!("  ⚠️ 标题可能是 URL token: {}", preview.title);
    }
    assert!(!preview.content_markdown.is_empty(), "正文不为空");
    println!("  ✅ 文档预览通过");
}

#[test]
fn test_b06_extract_doc_full() {
    println!("\n=== B06: 单文档提取（正文+图片，完整验证）===");

    let temp_dir = std::env::temp_dir().join("larkreader_full_test");
    std::fs::create_dir_all(&temp_dir).unwrap();

    let settings = models::Settings {
        output_dir: temp_dir.to_string_lossy().to_string(),
        concurrency: 3,
        download_images: true,
    };

    let result = extract::extract_doc(TEST_URL, &settings.output_dir, &settings).expect("提取失败");

    println!("  标题: {}", result.title);
    println!("  文件名: {}", result.filename);
    println!("  字符数: {}", result.char_count);
    println!(
        "  图片: 总{}张, 成功{}张, 失败{}张",
        result.image_count, result.images_downloaded, result.images_failed
    );
    println!("  状态: {:?}", result.status);
    println!("  文件路径: {}", result.filepath);

    // 基本验证
    assert!(!result.title.is_empty(), "标题不为空");
    assert!(result.char_count > 100, "字符数应该 > 100");
    assert!(result.image_count > 0, "这个文档应该有图片");
    assert_eq!(result.images_failed, 0, "图片下载不应该有失败");
    assert!(
        std::path::Path::new(&result.filepath).exists(),
        "文件必须存在"
    );

    // 验证图片文件存在
    let img_dir = std::path::Path::new(&result.filepath)
        .with_extension("")
        .to_string_lossy()
        .to_string()
        + "_images";
    let img_dir_path = std::path::Path::new(&img_dir);
    if std::path::Path::new(img_dir_path).exists() {
        let img_count = std::fs::read_dir(img_dir_path)
            .map(|d| d.count())
            .unwrap_or(0);
        println!("  图片目录文件数: {}", img_count);
        assert_eq!(
            img_count, result.images_downloaded,
            "图片目录文件数应该等于下载数"
        );
    }

    println!("  ✅ 单文档提取完整验证通过");
}

// ============================================================================
// C. 边界情况
// ============================================================================

#[test]
fn test_c01_empty_url() {
    println!("\n=== C01: 空链接 ===");

    let result = extract::preview_doc("");
    assert!(result.is_err(), "空链接应该返回错误");
    let err = result.unwrap_err().to_string();
    println!("  错误: {}", err);
    assert!(!err.is_empty(), "错误信息不应该为空");
    println!("  ✅ 空链接正确返回错误");
}

#[test]
fn test_c02_invalid_url() {
    println!("\n=== C02: 无效链接 ===");

    let result =
        extract::preview_doc("https://gcnyv4rcw1jv.feishu.cn/wiki/INVALID_TOKEN_NOT_EXIST_99999");
    assert!(result.is_err(), "无效链接应该返回错误");
    let err = result.unwrap_err().to_string();
    println!("  错误: {}", err);
    // 错误信息应该是中文
    let is_chinese = err.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    assert!(is_chinese, "错误信息应该包含中文");
    println!("  ✅ 无效链接返回中文错误");
}

#[test]
fn test_c03_pure_token() {
    println!("\n=== C03: 纯 token 输入 ===");

    // 纯 token 应该和完整 URL 效果一样
    let token = "Kh47wj3YRiPxsekidFWcbW0Knkb";
    let url = "https://gcnyv4rcw1jv.feishu.cn/wiki/Kh47wj3YRiPxsekidFWcbW0Knkb";

    let token_parsed = extract::parse_node_token(token);
    let url_parsed = extract::parse_node_token(url);

    assert_eq!(token_parsed, url_parsed, "纯 token 和 URL 解析结果应该一样");
    println!("  ✅ 纯 token 解析: {}", token_parsed);
}

#[test]
fn test_c04_url_with_query_params() {
    println!("\n=== C04: 带查询参数的 URL ===");

    let url = "https://gcnyv4rcw1jv.feishu.cn/wiki/Kh47wj3YRiPxsekidFWcbW0Knkb?source=test&x=1";
    let token = extract::parse_node_token(url);
    assert_eq!(token, "Kh47wj3YRiPxsekidFWcbW0Knkb", "应该去掉查询参数");
    println!("  ✅ 带查询参数的 URL 解析: {}", token);
}

#[test]
fn test_c05_url_with_trailing_slash() {
    println!("\n=== C05: 带尾斜杠的 URL ===");

    let url = "https://gcnyv4rcw1jv.feishu.cn/wiki/Kh47wj3YRiPxsekidFWcbW0Knkb/";
    let token = extract::parse_node_token(url);
    assert_eq!(token, "Kh47wj3YRiPxsekidFWcbW0Knkb", "应该去掉尾斜杠");
    println!("  ✅ 带尾斜杠的 URL 解析: {}", token);
}

// ============================================================================
// D. 错误处理验证
// ============================================================================

#[test]
fn test_d01_error_message_is_chinese() {
    println!("\n=== D01: 错误信息中文化 ===");

    // 用无效链接触发错误
    let result = extract::preview_doc("https://gcnyv4rcw1jv.feishu.cn/wiki/NOT_EXIST_12345");
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    let has_chinese = err.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    let has_raw_json = err.contains('{') || err.contains('\"');

    println!("  错误: {}", err);
    assert!(has_chinese, "错误信息必须包含中文");
    assert!(!has_raw_json, "错误信息不应该包含原始 JSON");
    println!("  ✅ 错误信息已中文化，无原始 JSON");
}

#[test]
fn test_d02_no_panic_on_garbage_url() {
    println!("\n=== D02: 垃圾输入不 panic ===");

    let garbage_urls = vec![
        "garbage",
        "https://example.com/not/wiki",
        "https://gcnyv4rcw1jv.feishu.cn/wiki/",
        "https://gcnyv4rcw1jv.feishu.cn/wiki/   ",
        "   ",
    ];

    for url in &garbage_urls {
        let result = extract::preview_doc(url);
        assert!(result.is_err(), "垃圾输入 \"{}\" 应该返回错误", url);
        let err = result.unwrap_err().to_string();
        assert!(!err.is_empty(), "错误信息不应该为空");
        println!("  ✅ \"{}\" → 错误（未 panic）", url);
    }
}

// ============================================================================
// E. 重复执行稳定性
// ============================================================================

#[test]
fn test_e01_repeat_extract_same_doc() {
    println!("\n=== E01: 同一文档连续提取两次（稳定性）===");

    let temp_dir = std::env::temp_dir().join("larkreader_repeat_test");
    std::fs::create_dir_all(&temp_dir).unwrap();

    let settings = models::Settings {
        output_dir: temp_dir.to_string_lossy().to_string(),
        concurrency: 3,
        download_images: true,
    };

    // 第一次提取
    let result1 =
        extract::extract_doc(TEST_URL, &settings.output_dir, &settings).expect("第一次提取失败");
    println!(
        "  第一次: 标题={}, 图片={}/{}, 状态={:?}",
        result1.title, result1.images_downloaded, result1.image_count, result1.status
    );
    assert_eq!(result1.images_failed, 0, "第一次不应该有图片失败");

    // 第二次提取（同一文档同一目录，测试旧文件清理）
    let result2 =
        extract::extract_doc(TEST_URL, &settings.output_dir, &settings).expect("第二次提取失败");
    println!(
        "  第二次: 标题={}, 图片={}/{}, 状态={:?}",
        result2.title, result2.images_downloaded, result2.image_count, result2.status
    );
    assert_eq!(result2.images_failed, 0, "第二次不应该有图片失败");

    // 两次结果应该一致
    assert_eq!(result1.title, result2.title, "两次提取标题应该一致");
    assert_eq!(
        result1.image_count, result2.image_count,
        "两次图片数应该一致"
    );
    assert_eq!(
        result1.images_downloaded, result2.images_downloaded,
        "两次下载数应该一致"
    );

    println!("  ✅ 重复提取稳定性验证通过");
}

#[test]
fn test_e02_extract_sub_doc() {
    println!("\n=== E02: 提取子文档（不同文档验证）===");

    let temp_dir = std::env::temp_dir().join("larkreader_subdoc_test");
    std::fs::create_dir_all(&temp_dir).unwrap();

    let settings = models::Settings {
        output_dir: temp_dir.to_string_lossy().to_string(),
        concurrency: 3,
        download_images: true,
    };

    let result =
        extract::extract_doc(SUB_DOC_URL, &settings.output_dir, &settings).expect("子文档提取失败");

    println!("  标题: {}", result.title);
    println!("  字符数: {}", result.char_count);
    println!("  图片: {}张", result.image_count);
    println!("  状态: {:?}", result.status);

    assert!(!result.title.is_empty(), "标题不为空");
    assert!(result.char_count > 0, "字符数 > 0");
    assert!(std::path::Path::new(&result.filepath).exists(), "文件存在");

    println!("  ✅ 子文档提取通过");
}

#[test]
fn test_e03_wiki_tree_structure() {
    println!("\n=== E03: 知识库目录树结构验证 ===");

    let tree = wiki::get_wiki_tree(TEST_URL).expect("获取目录树失败");

    println!("  根节点: {}", tree.title);
    println!("  子节点数: {}", tree.children.len());
    println!("  文档总数: {}", tree.count_docs());

    // 基本验证
    assert!(!tree.title.is_empty(), "根节点标题不为空");
    assert!(tree.has_child, "根节点应该有子节点");
    assert!(!tree.children.is_empty(), "子节点列表不为空");
    assert!(tree.count_docs() > 0, "文档总数 > 0");

    // 验证子节点排序
    for i in 1..tree.children.len() {
        assert!(
            tree.children[i - 1].position <= tree.children[i].position,
            "子节点应该按 position 排序"
        );
    }

    println!("  ✅ 目录树结构验证通过");
}
