#!/usr/bin/env node
// ============================================================================
// scripts/gen-latest.mjs —— 生成应用内更新的清单 latest.json（本机发版用）
//
// 用法（项目根目录）：
//   node scripts/gen-latest.mjs <版本，如 0.3.0> ["本版更新说明"]
//
// 读取 src-tauri/target/release/bundle/nsis/LarkReader_<版本>_x64-setup.exe.sig，
// 写出 src-tauri/target/release/bundle/latest.json（version / pub_date / 平台签名 + 下载地址）。
//
// 前置：先跑 `npm run tauri build`（带 TAURI_SIGNING_PRIVATE_KEY），否则没有 .sig 侧车。
// 说明文字省略时用一句通用文案；建议发版时传入本版更新要点。
// ============================================================================

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const BUNDLE = path.join(ROOT, "src-tauri", "target", "release", "bundle");

const version = (process.argv[2] || "").replace(/^v/, "");
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error('用法：node scripts/gen-latest.mjs <版本，如 0.3.0> ["本版更新说明"]');
  process.exit(1);
}

const notes =
  process.argv[3] ||
  `LarkReader v${version} 更新。详见 https://github.com/LPK3215/LarkReader/releases/tag/v${version}`;

const nsisName = `LarkReader_${version}_x64-setup.exe`;
const sigPath = path.join(BUNDLE, "nsis", `${nsisName}.sig`);
if (!fs.existsSync(sigPath)) {
  console.error(
    `找不到签名侧车：${sigPath}\n先执行 npm run tauri build（并设置 TAURI_SIGNING_PRIVATE_KEY）再来生成清单。`
  );
  process.exit(1);
}

const manifest = {
  notes,
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  platforms: {
    "windows-x86_64": {
      signature: fs.readFileSync(sigPath, "utf8").trim(),
      url: `https://github.com/LPK3215/LarkReader/releases/download/v${version}/${nsisName}`,
    },
  },
  version,
};

const out = path.join(BUNDLE, "latest.json");
fs.writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
console.log(`已生成 ${path.relative(ROOT, out)}（version=${version}）`);
