# 项目脚本

仓库根目录的常用操作都收敛到几条命令。脚本本体在 `scripts/`（Node.js 实现，跨平台可用）。

| npm 命令 | 脚本 | 作用 |
|---|---|---|
| `npm run verify` | `scripts/verify.mjs` | **发布门禁**：cargo fmt / clippy / lib 单测 + 前端 vue-tsc + vite build，任一失败即停 |
| `npm run clean:e2e` | `scripts/clean-e2e.mjs` | 清空 `e2e_download_tmp/`、`e2e_download_tmp_big/`（保留空目录），跑下载回归前必做 |
| `npm run release -- <版本>` | `scripts/release.mjs` | 一键发版：跑门禁 → 自动回写三处版本号并提交 → 打 `v<版本>` tag → 推分支 + tag（本仓库 CI 出包不可用，产物发布走本机 CLI 路径，见 `docs/release-and-update.md` §0） |
| `node scripts/gen-latest.mjs <版本> "说明"` | `scripts/gen-latest.mjs` | 生成应用内更新清单 `latest.json`（读 NSIS `.sig` 侧车 + 拼下载地址）；本机发版第 4 步用 |

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
- 发版走 `npm run release -- <版本>`（门禁 + 三处版本回写 + 打 tag + 推分支与 tag）；**产物发布走本机 CLI 路径**：`npm run tauri build`（带签名）→ `node scripts/gen-latest.mjs <版本>` → `gh release create <tag> --latest <产物>`。本仓库的 GitHub Actions 因**账号级**限制无法启动 runner（`.github/workflows/publish.yml` 即便被 tag 触发也跑不起来，报 `account is locked due to a billing issue`；仓库是 public，Actions 分钟数本就免费，与使用方式无关），故以本机路径为准。完整顺序见 `docs/release-and-update.md` §0。
- 包管理器统一为 **npm**（曾用 pnpm，`pnpm tauri add` 会破坏 node_modules 结构，已弃用）。
