#!/usr/bin/env node
// ============================================================================
// scripts/verify.mjs —— 本地全量发布门禁
//
// 依序执行：
//   Rust : cargo fmt --check / cargo clippy --all-targets -D warnings / cargo test --lib
//   Web  : npm run build（= vue-tsc --noEmit && vite build）
//
// 任何一步失败即停并汇总失败清单，退出码非 0。
//
// 用法：
//   npm run verify            # 全量（默认）
//   node scripts/verify.mjs --rust-only   # 只查 Rust
//   node scripts/verify.mjs --web-only    # 只查前端
//
// 说明：真实下载类集成测试（src-tauri/tests/z_tmp_*，需登录飞书并联网）
//   不在门禁内，需要时手动跑：cd src-tauri && cargo test --test z_tmp_full_download
// ============================================================================

import { execSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const TAURI = path.join(ROOT, "src-tauri");

const args = process.argv.slice(2);
const rustOnly = args.includes("--rust-only");
const webOnly = args.includes("--web-only");

const failed = [];

function run(step, cmd, cwd) {
  console.log(`\n=== [${step}] ===`);
  try {
    execSync(cmd, { cwd, stdio: "inherit" });
    console.log(`--- ${step} 通过 ---`);
    return true;
  } catch {
    failed.push(step);
    return false;
  }
}

if (!webOnly) {
  run("cargo fmt --check", "cargo fmt --check", TAURI);
  run("cargo clippy --all-targets", "cargo clippy --all-targets -- -D warnings", TAURI);
  run("cargo test --lib", "cargo test --lib", TAURI);
}

if (!rustOnly) {
  run("npm run build", process.platform === "win32" ? "npm.cmd run build" : "npm run build", ROOT);
}

if (failed.length > 0) {
  console.error(`\n[verify] 以下 ${failed.length} 项失败：\n  - ${failed.join("\n  - ")}`);
  process.exit(1);
} else {
  console.log("\n[verify] 全绿：fmt / clippy / lib 单测 / 前端构建均通过。");
}
