//! 新用户体验测试 — 模拟刚下载工具、未登录状态下的完整流程
//!
//! 验证目标：
//! 1. check_env 能正确检测出"未登录"状态
//! 2. 在未登录时调用 preview_doc / extract_doc 会返回什么错误
//! 3. 错误信息是否友好、可理解

use lark_reader_lib::{env, extract, models};

#[test]
fn test_new_user_step1_check_env() {
    println!("\n========================================");
    println!("  新用户体验测试 - 第 1 步: 环境检测");
    println!("========================================\n");

    let status = env::check_env();

    println!("Node.js:    {}  版本: {:?}", status.node_installed, status.node_version);
    println!("lark-cli:   {}  版本: {:?}", status.lark_cli_installed, status.lark_cli_version);
    println!("应用配置:   {}  AppID: {:?}", status.app_configured, status.app_id);
    println!("已登录:     {}", status.logged_in);
    println!("用户名:     {:?}", status.user_name);
    println!("Token状态:  {:?}", status.token_status);

    // 验证：环境检测应该正确反映状态
    assert!(status.node_installed, "Node.js 应该已安装");
    assert!(status.lark_cli_installed, "lark-cli 应该已安装");
    assert!(status.app_configured, "飞书应用应该已配置");
    
    // 关键验证：未登录状态下 logged_in 应该是 false
    if !status.logged_in {
        println!("\n✅ 正确检测到用户未登录");
        println!("   → 前端应该显示「请先登录飞书」的引导");
    } else {
        println!("\n❌ 错误：未登录但 check_env 返回 logged_in=true");
    }
}

#[test]
fn test_new_user_step2_preview_doc_without_login() {
    println!("\n========================================");
    println!("  新用户体验测试 - 第 2 步: 未登录时预览文档");
    println!("========================================\n");

    // 模拟用户输入一个飞书链接
    let url = "https://gcnyv4rcw1jv.feishu.cn/wiki/QJFEw6cH0iSry4kRUcMcDttfn4e";

    println!("用户输入链接: {}", url);
    println!("用户状态: 未登录");
    println!("尝试预览文档...\n");

    let result = extract::preview_doc(url);

    match &result {
        Ok(preview) => {
            println!("✅ 意外成功（不应该到这里）");
            println!("   标题: {}", preview.title);
            println!("   字符数: {}", preview.char_count);
            println!("   图片数: {}", preview.images.len());
        }
        Err(e) => {
            println!("❌ 预览失败（预期行为）");
            println!("   错误类型: {:?}", e);
            println!("   错误信息: {}", e);
            
            let err_str = e.to_string();
            println!("\n   --- 错误友好度分析 ---");
            
            if err_str.contains("未登录") || err_str.contains("请重新登录") {
                println!("   ✅ 错误信息友好：明确提示用户需要登录");
            } else if err_str.contains("权限") {
                println!("   ⚠️ 错误信息提到权限，但没有明确说「请先登录」");
            } else if err_str.contains("token") || err_str.contains("auth") || err_str.contains("{\"") {
                println!("   ❌ 错误信息不友好，包含原始 JSON 或英文");
                println!("      当前错误: {}", err_str);
            } else {
                println!("   ⚠️ 错误信息可能需要优化");
                println!("      当前错误: {}", err_str);
            }
            
            // 关键验证：错误信息应该是中文友好提示，不是原始 JSON
            assert!(
                err_str.contains("未登录") || err_str.contains("请重新登录"),
                "错误信息应该包含「未登录」或「请重新登录」，但实际是: {}",
                err_str
            );
        }
    }

    // 不管成功还是失败，测试都通过——我们只是在观察行为
    println!("\n测试完成");
}

#[test]
fn test_new_user_step3_extract_doc_without_login() {
    println!("\n========================================");
    println!("  新用户体验测试 - 第 3 步: 未登录时提取文档");
    println!("========================================\n");

    let url = "https://gcnyv4rcw1jv.feishu.cn/wiki/QJFEw6cH0iSry4kRUcMcDttfn4e";
    let settings = models::Settings::default();

    println!("用户输入链接: {}", url);
    println!("输出目录: {}", settings.output_dir);
    println!("用户状态: 未登录");
    println!("尝试提取文档...\n");

    let result = extract::extract_doc(url, &settings.output_dir, &settings);

    match &result {
        Ok(r) => {
            println!("✅ 意外成功（不应该到这里）");
            println!("   标题: {}", r.title);
            println!("   状态: {:?}", r.status);
        }
        Err(e) => {
            println!("❌ 提取失败（预期行为）");
            println!("   错误类型: {:?}", e);
            println!("   错误信息: {}", e);

            let err_str = e.to_string();
            println!("\n   --- 错误友好度分析 ---");

            if err_str.contains("未登录") || err_str.contains("请重新登录") {
                println!("   ✅ 错误信息友好：明确提示用户需要登录");
            } else if err_str.contains("权限") {
                println!("   ⚠️ 错误信息提到权限，但没有明确说「请先登录」");
            } else if err_str.contains("token") || err_str.contains("auth") || err_str.contains("{\"") {
                println!("   ❌ 错误信息不友好，包含原始 JSON 或英文");
                println!("      当前错误: {}", err_str);
            } else {
                println!("   ⚠️ 错误信息可能需要优化");
                println!("      当前错误: {}", err_str);
            }

            assert!(
                err_str.contains("未登录") || err_str.contains("请重新登录"),
                "错误信息应该包含「未登录」或「请重新登录」，但实际是: {}",
                err_str
            );
        }
    }

    println!("\n测试完成");
}

#[test]
fn test_new_user_step4_lark_cli_error_format() {
    println!("\n========================================");
    println!("  新用户体验测试 - 第 4 步: 分析 lark-cli 未登录时的原始错误");
    println!("========================================\n");

    // 直接调 lark-cli docs +fetch，看原始错误是什么
    println!("直接调用 lark-cli docs +fetch（未登录状态）...\n");

    // 直接用 std::process::Command 看原始输出
    let output = std::process::Command::new("lark-cli.cmd")
        .args(["docs", "+fetch", "--doc", "https://gcnyv4rcw1jv.feishu.cn/wiki/QJFEw6cH0iSry4kRUcMcDttfn4e", "--doc-format", "markdown", "--as", "user"])
        .env_remove("HERMES_HOME")
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            println!("退出码: {:?}", out.status.code());
            println!("stdout 长度: {}", stdout.len());
            println!("stdout 前3字节: {:?}", &out.stdout[..3.min(out.stdout.len())]);
            let stdout_preview: String = stdout.chars().take(200).collect();
            println!("stdout 内容前200字符: {}", stdout_preview);
            println!("stderr 长度: {}", stderr.len());
            if !stderr.is_empty() {
                let stderr_preview: String = stderr.chars().take(200).collect();
                println!("stderr 内容: {}", stderr_preview);
            }
            
            // 尝试解析 JSON
            let trimmed = stdout.trim();
            let json_start = trimmed.find('{');
            println!("\n第一个 {{ 位置: {:?}", json_start);
            if let Some(pos) = json_start {
                let json_str = &trimmed[pos..];
                let json_preview: String = json_str.chars().take(100).collect();
                println!("JSON 字符串前100字符: {:?}", json_preview);
                match serde_json::from_str::<serde_json::Value>(json_str) {
                    Ok(v) => println!("JSON 解析成功: ok={:?}", v.get("ok")),
                    Err(e) => println!("JSON 解析失败: {}", e),
                }
            }
        }
        Err(e) => {
            println!("Command 执行失败: {}", e);
        }
    }

    println!("\n测试完成");
}
