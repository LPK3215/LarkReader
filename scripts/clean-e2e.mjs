#!/usr/bin/env node
// ============================================================================
// scripts/clean-e2e.mjs —— 清空 E2E 测试工作目录（保留空目录）
//
// 约定（见 docs/BACKEND.md 与 docs/e2e-download-case/README.md）：
//   e2e_download_tmp/、e2e_download_tmp_big/ 是真实下载回归的临时输出，
//   已被 .gitignore 忽略、不入库；每次跑下载测试前必须清空，避免旧产物干扰断言。
//   目录本身保留。
//
// 用法：
//   npm run clean:e2e
// ============================================================================

import { existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

const targets = ["e2e_download_tmp", "e2e_download_tmp_big"];

for (const name of targets) {
  const dir = path.join(ROOT, name);
  if (!existsSync(dir)) {
    console.log(`[clean-e2e] ${name}/ 不存在，已跳过。`);
    continue;
  }
  const before = readdirSync(dir).length;
  for (const child of readdirSync(dir)) {
    rmSync(path.join(dir, child), { recursive: true, force: true });
  }
  mkdirSync(dir, { recursive: true });
  console.log(`[clean-e2e] ${name}/ 已清空（移除 ${before} 项），目录保留。`);
}

console.log("[clean-e2e] 完成。");
