# 项目脚本

仓库根目录的常用操作都收敛到几条命令。脚本本体在 `scripts/`（Node.js 实现，跨平台可用）。

| npm 命令 | 脚本 | 作用 |
|---|---|---|
| `npm run verify` | `scripts/verify.mjs` | **发布门禁**：cargo fmt / clippy / lib 单测 + 前端 vue-tsc + vite build，任一失败即停 |
| `npm run clean:e2e` | `scripts/clean-e2e.mjs` | 清空 `e2e_download_tmp/`、`e2e_download_tmp_big/`（保留空目录），跑下载回归前必做 |
| `npm run release -- <版本>` | `scripts/release.mjs` | 一键发版：跑门禁 → 自动回写三处版本号并提交 → 打 `v<版本>` tag → 推分支 + tag → 触发 GitHub Actions 三平台出包 |

## 手动命令（未封装，按需使用）

```powershell
# 本地开发（带 Rust 后端的桌面窗口）
npm run tauri dev

# 纯网页预览（无后端，mock 数据演示）
npm run dev

# 后端集成测试（真实下载类，需已登录飞书 + 联网）
cd src-tauri
cargo test --test z_tmp_big_download
```

## 约定

- 真实下载回归测试前，先 `npm run clean:e2e` 清空输出目录（测试脚本也会覆盖写入，但干净起点更保险）。
- 发版走 `npm run release`：GitHub Actions 的 workflow（`.github/workflows/publish.yml`）监听 `v*` tag，产物直接**正式发布**（draft 不属于 latest，应用内更新拉不到）。发版前需先把签名私钥配成仓库 secret `TAURI_SIGNING_PRIVATE_KEY`。完整细节见 `docs/release-and-update.md`。
- 包管理器统一为 **npm**（曾用 pnpm，`pnpm tauri add` 会破坏 node_modules 结构，已弃用）。
