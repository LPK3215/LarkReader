#!/usr/bin/env node
// ============================================================================
// scripts/release.mjs —— 一键打版本 tag 并推送，触发 GitHub Actions 三平台出包
//
// 流程：
//   1. 检查工作区无未提交改动（除非 --force，发布应从干净历史出发）
//   2. 默认先跑 scripts/verify.mjs 发布门禁（--no-verify 跳过）
//   3. 把新版本号自动回写 package.json / src-tauri/tauri.conf.json /
//      src-tauri/Cargo.toml（三处一致，保证 CI 出包版本与 tag 相同）
//   4. 提交版本回写 commit，打 v<版本> tag 并推送当前分支 + tag
//   5. GitHub Actions .github/workflows/publish.yml 收到 tag 后自动构建
//      Windows/macOS/Linux 三平台安装包，产物进正式 GitHub Release
//
// 用法（在项目根目录）：
//   npm run release -- 0.2.0            # 普通发布（先跑门禁）
//   node scripts/release.mjs 0.2.0 --no-verify
//   node scripts/release.mjs 0.2.0 --force   # 工作区有改动也强行发布
//   node scripts/release.mjs 0.2.0 --dry-run # 只预览会回写哪些版本文件，不写盘不提交不推送
// ============================================================================

import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const PKG_JSON = path.join(ROOT, "package.json");
const TAURI_CONF = path.join(ROOT, "src-tauri", "tauri.conf.json");
const CARGO_TOML = path.join(ROOT, "src-tauri", "Cargo.toml");

const args = process.argv.slice(2);
const versionArg = args.find((a) => !a.startsWith("--"));
const force = args.includes("--force");
const dryRun = args.includes("--dry-run");
const skipVerify = args.includes("--no-verify") || dryRun; // 预览模式不跑慢门禁

function sh(cmd, opts = {}) {
  console.log(`\n$ ${cmd}`);
  execSync(cmd, { cwd: ROOT, stdio: "inherit", ...opts });
}

if (!versionArg) {
  console.error("用法：node scripts/release.mjs <版本，如 0.2.0> [--no-verify] [--force] [--dry-run]");
  process.exit(1);
}

const version = versionArg.replace(/^v/, "");
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`版本号格式错误：${versionArg}（应为 0.2.0 或 v0.2.0）`);
  process.exit(1);
}
const tag = `v${version}`;

/** 计算 JSON 版本文件（package.json / tauri.conf.json）的新内容；同版本返回 null */
function bumpJson(file, nextVersion) {
  const raw = fs.readFileSync(file, "utf8");
  const obj = JSON.parse(raw);
  if (typeof obj.version !== "string") {
    throw new Error(`${file} 缺少顶层 version 字段，无法回写`);
  }
  if (obj.version === nextVersion) return null;
  const old = obj.version;
  obj.version = nextVersion;
  return { file, old, next: `${JSON.stringify(obj, null, 2)}\n` };
}

/** 计算 Cargo.toml 在 [package] 段的 version 行新内容；同版本返回 null */
function bumpCargo(file, nextVersion) {
  const raw = fs.readFileSync(file, "utf8");
  const marker = raw.indexOf("[package]");
  if (marker < 0) throw new Error(`${file} 找不到 [package] 段`);
  const rest = raw.slice(marker);
  const m = rest.match(/^version = "([^"]*)"/m);
  if (!m) throw new Error(`${file} 的 [package] 段里没有 version 行`);
  if (m[1] === nextVersion) return null;
  const next = rest.replace(/^version = "[^"]*"/m, `version = "${nextVersion}"`);
  return { file, old: m[1], next: raw.slice(0, marker) + next };
}

function ensureGitIdentity() {
  try {
    execSync("git config user.name", { cwd: ROOT, stdio: "pipe" });
    execSync("git config user.email", { cwd: ROOT, stdio: "pipe" });
  } catch {
    console.error(
      "\n本机 git 没有配置 user.name / user.email，无法自动提交版本回写。\n" +
        "先执行：git config user.name \"你的名字\" && git config user.email \"you@example.com\""
    );
    process.exit(1);
  }
}

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

// 4) 版本号自动回写三处（保证 CI 产物与 tag 一致）
const bumps = [
  bumpJson(PKG_JSON, version),
  bumpCargo(CARGO_TOML, version),
  bumpJson(TAURI_CONF, version),
].filter(Boolean);

if (bumps.length === 0) {
  console.log(`\n[release] 三处版本号已是 ${version}，无需回写。`);
} else {
  console.log(`\n[release] 把版本号回写为 ${version}：`);
  for (const b of bumps) {
    console.log(`  - ${path.relative(ROOT, b.file)}: ${b.old} -> ${version}`);
  }
}

if (dryRun) {
  console.log("\n[dry-run] 以上仅为预览：未写盘、未提交、未推送。");
  process.exit(0);
}

if (bumps.length > 0) {
  ensureGitIdentity();
  for (const b of bumps) fs.writeFileSync(b.file, b.next);
  sh(`git add -- "${PKG_JSON}" "${CARGO_TOML}" "${TAURI_CONF}"`);
  sh(`git commit -m "chore: bump version to v${version}"`);
}

// 5) 打 tag + 推送
sh(`git tag "${tag}"`);
const branch = execSync("git rev-parse --abbrev-ref HEAD", { cwd: ROOT }).toString().trim();
sh(`git push origin "${branch}"`);
sh(`git push origin "${tag}"`);

console.log(`\n[release] ${tag} 已推送。GitHub Actions 正在三平台构建，产物将进入正式 Release：`);
console.log("  https://github.com/LPK3215/LarkReader/releases");
