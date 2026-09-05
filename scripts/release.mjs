#!/usr/bin/env node
// ============================================================================
// scripts/release.mjs —— 一键打版本 tag 并推送，触发 GitHub Actions 三平台出包
//
// 流程：
//   1. 检查工作区无未提交改动（除非 --force，发布应从干净历史出发）
//   2. 默认先跑 scripts/verify.mjs 发布门禁（--no-verify 跳过）
//   3. git tag v<版本> 并推送当前分支 + tag
//   4. GitHub Actions .github/workflows/publish.yml 收到 tag 后自动构建
//      Windows/macOS/Linux 三平台安装包，产物进 draft release
//
// 用法（在项目根目录）：
//   npm run release -- 0.2.0            # 普通发布（先跑门禁）
//   node scripts/release.mjs 0.2.0 --no-verify
//   node scripts/release.mjs 0.2.0 --force   # 工作区有改动也强行发布
// ============================================================================

import { execSync } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");

const args = process.argv.slice(2);
const versionArg = args.find((a) => !a.startsWith("--"));
const force = args.includes("--force");
const skipVerify = args.includes("--no-verify");

function sh(cmd, opts = {}) {
  console.log(`\n$ ${cmd}`);
  execSync(cmd, { cwd: ROOT, stdio: "inherit", ...opts });
}

if (!versionArg) {
  console.error("用法：node scripts/release.mjs <版本，如 0.2.0> [--no-verify] [--force]");
  process.exit(1);
}

const version = versionArg.replace(/^v/, "");
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`版本号格式错误：${versionArg}（应为 0.2.0 或 v0.2.0）`);
  process.exit(1);
}
const tag = `v${version}`;

// 1) 工作区状态
const status = execSync("git status --porcelain", { cwd: ROOT }).toString().trim();
if (status && !force) {
  console.error(
    "工作区有未提交改动，发布应从干净历史出发。\n" +
      "先提交或撤销改动，或用 --force 强行发布（风险自负）。\n\n" +
      status
  );
  process.exit(1);
}

// 2) 门禁
if (!skipVerify) {
  sh(process.platform === "win32" ? "npm.cmd run verify" : "npm run verify");
}

// 3) tag 已存在检查
try {
  execSync(`git rev-parse "${tag}"`, { cwd: ROOT, stdio: "ignore" });
  console.error(`tag ${tag} 已存在。删除旧 tag（git tag -d ${tag} && git push origin :${tag}）后再发。`);
  process.exit(1);
} catch {
  /* tag 不存在，继续 */
}

// 4) 打 tag + 推送
sh(`git tag "${tag}"`);
const branch = execSync("git rev-parse --abbrev-ref HEAD", { cwd: ROOT }).toString().trim();
sh(`git push origin "${branch}"`);
sh(`git push origin "${tag}"`);

console.log(`\n[release] ${tag} 已推送。GitHub Actions 正在三平台构建，产物将进入 draft release：`);
console.log("  https://github.com/LPK3215/LarkReader/releases");
console.log("如工作区有未提交的文档/脚本改动，记得另行提交推送。");
