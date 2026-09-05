# 构建 · 发布 · 应用内更新

本文档是 LarkReader 的完整部署手册，覆盖：

1. 环境准备（首次构建前的依赖清单与命令）
2. 本地常用命令速查（开发 / 门禁 / 打包）
3. 本地打包的步骤与产物位置（含 updater 签名注意事项）
4. 版本号管理（为什么 tag 与产物必须一致、如何自动化）
5. 通过 GitHub Actions 多平台出包与 Release 发布
6. 构建产物的命名清单
7. 应用内「检查更新」链路
8. 签名密钥（公钥 / 私钥职责、配置、安全红线）
9. 发布门禁与本地验证
10. 常见问题排查
11. 相关文件索引

---

## 1. 环境准备（第一次构建前）

### 1.1 通用工具

| 工具 | 版本要求 | 说明 |
|---|---|---|
| Node.js + npm | Node 22（CI 同款） | 前端依赖与 Vite 构建 |
| Rust toolchain | stable（`rustup default stable`） | Tauri 后端 |
| Git | 任意较新版本 | 代码 / tag 推送 |

### 1.2 各平台系统依赖

**Windows**
- WebView2 运行时：Win11 / 新 Win10 自带；旧系统需装 [Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。
- Visual Studio Build Tools（勾选 “Desktop development with C++”，含 MSVC x64 工具链）。
- 默认构建目标 `x86_64-pc-windows-msvc`。

**Ubuntu / Debian**
```bash
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf \
  build-essential \
  curl wget file
```

**macOS**
```bash
xcode-select --install   # Xcode Command Line Tools
```
- Apple Silicon 目标：`rustup target add aarch64-apple-darwin`
- Intel 目标：`rustup target add x86_64-apple-darwin`

### 1.3 安装项目依赖

```bash
npm install        # 首次；或用 npm ci（按 lockfile 精确安装）
```

---

## 2. 本地常用命令速查

| 场景 | 命令 |
|---|---|
| 启动开发模式（带热更新，前端 + 后端） | `npm run tauri dev` |
| 仅前端类型检查 + 构建 | `npm run build` |
| 发布全量门禁（fmt/clippy/test/build） | `npm run verify` |
| 只查 Rust | `node scripts/verify.mjs --rust-only` |
| 只查前端 | `node scripts/verify.mjs --web-only` |
| 打包当前平台 | `npm run tauri build` |
| 预览一次发版会改哪些文件 | `npm run release -- 0.2.0 --dry-run` |
| 正式发版并触发 CI | `npm run release -- 0.2.0` |

> `npm run tauri dev` 会自动执行 `package.json` 里配置的 `beforeDevCommand`（即 `npm run dev`，Vite 起在 `http://localhost:1420`）。

---

## 3. 本地打包：步骤与产物位置

### 3.1 打包前的签名准备（重要）

启用应用内更新后（`tauri.conf.json` 已内置公钥），**打包阶段必须提供私钥给安装包签名**，否则打包会失败。私钥读取两种方式二选一：

方式 A —— 环境变量指向密钥文件（推荐本机）：
```powershell
# Windows PowerShell
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "$env:USERPROFILE\.larkreader-signing\larkreader.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""   # 仅当生成时设过密码才需要
```

方式 B —— 直接给私钥字符串：
```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.larkreader-signing\larkreader.key" -Raw
```

### 3.2 打包命令

默认打包当前平台的所有 bundle 类型（Windows 出 nsis+msi，Linux 出 deb+appimage，macOS 出 dmg 等）：

```bash
npm run tauri build
```

只出某一种安装包（Tauri 2 CLI 用 `--bundles`）：

```bash
# Windows 只出 NSIS（update 主力）
npm run tauri build -- --bundles nsis
# Linux 同时出 deb + AppImage
npm run tauri build -- --bundles deb,appimage
# macOS 只出 dmg
npm run tauri build -- --bundles dmg
```

指定架构（同一台 macOS 上分别打两种芯片包）：

```bash
npm run tauri build -- --target aarch64-apple-darwin   # M 系列
npm run tauri build -- --target x86_64-apple-darwin    # Intel
```

### 3.3 产物输出目录

```
src-tauri/target/release/bundle/
├── nsis/        LarkReader_x.y.z_x64-setup.exe
├── msi/         LarkReader_x.y.z_x64_en-US.msi
├── dmg/         LarkReader_x.y.z_aarch64.dmg / …_x64.dmg
├── appimage/    LarkReader_x.y.z_amd64.AppImage
├── deb/         larkreader_x.y.z_amd64.deb
└── … 以及各安装包对应的 .sig 签名侧车
```

> Tauri 的 GUI 依赖系统 WebView，跨操作系统交叉编译通常不可行——各平台包要在对应系统上构建。这正是 CI 用四平台矩阵的原因。

---

## 4. 版本号：tag 与产物必须一致

产物版本号不来自 tag，而是来自工程配置。Tauri 打包、CI 命名、updater 对比版本读的都是下面三处，**它们必须一致**：

- `package.json` → 前端包版本
- `src-tauri/tauri.conf.json` → 打包产物版本（`version` 字段）
- `src-tauri/Cargo.toml` → Rust crate 版本（写入日志、应用内版本展示）

只打 tag 不改三处会出现「tag 是 0.2.0、安装包却是 0.1.0」的错位。`scripts/release.mjs` 已自动回写：

```bash
npm run release -- 0.2.0
```

内部步骤：
1. 校验版本号格式（`x.y.z`）；
2. 检查工作区干净（有未提交改动会拒绝，除非 `--force`）；
3. 运行 `npm run verify` 门禁（`--no-verify` 跳过）；
4. 检查 `v0.2.0` tag 是否已存在；
5. 自动回写三个版本文件；
6. 提交 `chore: bump version to v0.2.0`；
7. 打 tag `v0.2.0` 并推送分支与 tag → 触发 CI。

---

## 5. 通过 GitHub Actions 发布 Release

### 5.1 一次性前置配置

在仓库 **Settings → Secrets and variables → Actions** 新建 secret：

- **name**：`TAURI_SIGNING_PRIVATE_KEY`
- **value**：`%USERPROFILE%\.larkreader-signing\larkreader.key` 文件的**完整内容**

配置完成后 CI 的 guard job 会通过；未配置时 guard 直接标红并给出指引，避免四平台白跑。

### 5.2 触发与流程

`.github/workflows/publish.yml` 触发条件：

- push tag `v*`（即 `npm run release` 自动完成）；
- Actions 页面手动 `workflow_dispatch`。

流程：
1. `guard`：校验签名 secret 就位；
2. `publish-tauri`（needs: guard）四平台矩阵出包：
   - mac aarch64 / mac x86_64 / ubuntu-22.04 / windows-latest；
3. `tauri-action` 把产物上传到对应 tag 的 Release 并**正式发布**（`releaseDraft: false`），命名 `LarkReader v__VERSION__`。

> 为什么不是 draft？应用内更新端点读 `/releases/latest/download/latest.json`，draft / prerelease 不属于 latest，用户永远检查不到新版本。是否发版由 `npm run release` 决定，日常开发推送不会触发本工作流。

### 5.3 查看发布结果

构建进度：仓库 **Actions** 页 → publish workflow。
发布产物：仓库 **Releases** 页 → `v0.2.0`。

---

## 6. 构建产物命名清单

产品名 **LarkReader**，标识 `com.larkreader.app`。命名规则：`产品名_版本号_架构.格式`（`.deb` 按 Debian 规范全小写）。

| 平台 | 架构 | 以 v0.1.0 为例 |
|---|---|---|
| Windows | x64 | `LarkReader_0.1.0_x64-setup.exe`（NSIS）<br>`LarkReader_0.1.0_x64_en-US.msi`（MSI） |
| macOS | Apple Silicon | `LarkReader_0.1.0_aarch64.dmg` |
| macOS | Intel | `LarkReader_0.1.0_x64.dmg` |
| Linux | amd64 | `larkreader_0.1.0_amd64.deb`<br>`LarkReader_0.1.0_amd64.AppImage` |

每个 Release 还会附带应用内更新所需文件：

| 文件 | 作用 |
|---|---|
| `*.sig`（与安装包一一对应） | 安装包的 ed25519 签名，客户端用内嵌公钥校验 |
| `latest.json` | 版本清单（版本号 / 下载地址 / 签名 / 时间），检查更新的依据 |

精确文件名（如 MSI 语言后缀）以 CI 实际产物为准。macOS 未做 Apple 签名，跨机器首次运行需在系统设置允许（零成本取舍）。

---

## 7. 应用内更新链路

### 7.1 更新源配置（`src-tauri/tauri.conf.json`）

```jsonc
"plugins": {
  "updater": {
    "pubkey": "…minisign 公钥…",                 // 验签公钥，随代码分发
    "endpoints": [
      "https://github.com/LPK3215/LarkReader/releases/latest/download/latest.json"
    ],
    "windows": { "installMode": "passive" }
  }
}
```

`bundle.createUpdaterArtifacts: true` 让打包阶段产出签名与更新清单。

### 7.2 客户端行为

- **启动时**：`src/App.vue` 调 `notifyUpdateOnce()` 静默检查一次，有新版才提示，且每进程只提示一次；提示指向「设置 → 软件更新」。
- **设置页**「软件更新」卡片（`src/views/SettingsView.vue`）：当前版本 → 检查更新 → 有新版则「下载并安装 vX」+ 进度条 → 自动安装并重启。
- 逻辑封装在 `src/api/updater.ts`。
- Windows：passive 安装器自动完成并重启；macOS/Linux：`tauri-plugin-process` relaunch 重启。
- 开发模式（`npm run dev`）下检查失败会被静默吞掉，不影响开发。

### 7.3 平台支持

| 平台 | 说明 |
|---|---|
| Windows | NSIS 自动更新（MSI 作安装备用包，不参与更新侧车） |
| Linux | AppImage 参与更新 |
| macOS | 机制可用；未做 Apple 签名，跨机器需系统放行 |

---

## 8. 签名密钥

生成位置（本机，**不在仓库**）：

```
%USERPROFILE%\.larkreader-signing\larkreader.key        # 私钥
%USERPROFILE%\.larkreader-signing\larkreader.key.pub    # 公钥
```

生成命令（仅首次需要）：

```powershell
# Windows PowerShell
$dir = Join-Path $env:USERPROFILE '.larkreader-signing'
New-Item -ItemType Directory -Force -Path $dir | Out-Null
npx tauri signer generate -w (Join-Path $dir 'larkreader.key') --ci
```

| 谁 | 放哪里 | 能否公开 |
|---|---|---|
| **私钥** | GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY` + 本地私密备份 | **绝不能**提交进仓库 / 发给任何人 |
| **公钥** | `tauri.conf.json` 的 `plugins.updater.pubkey` | 可以公开 |

> ⚠️ 私钥丢失 = 无法再给老用户签发更新包（公钥固定）。请立即把 `larkreader.key` 备份到安全介质。生成时未设密码；若设了密码，CI 还需要 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secret。

---

## 9. 发布门禁与本地验证

```bash
npm run verify                             # fmt + clippy + lib 单测 + 前端构建
node scripts/release.mjs 0.2.0 --dry-run  # 预览版本回写，不写盘不提交不推送
```

真实验证「自动更新闭环」的唯一方式：
1. 配置好 `TAURI_SIGNING_PRIVATE_KEY` secret；
2. `npm run release -- 0.2.0` 发一个真实版本；
3. 在低版本机器上打开应用 → 收到提示 → 下载安装 → 重启后版本变化。

此步依赖真实 tag 与 GitHub 远端，无法本地自证。

---

## 10. 常见问题排查

| 现象 | 原因 / 处理 |
|---|---|
| `npm run release` 报“工作区有未提交改动” | 先提交；确要带脏区发版加 `--force`（风险自负） |
| 报 `tag v0.2.0 已存在` | 换版本号；或 `git tag -d v0.2.0 && git push origin :v0.2.0` 删旧 tag（慎用） |
| 本地 `npm run tauri build` 报签名相关错误 | 忘了给 `TAURI_SIGNING_PRIVATE_KEY(PATH)` 环境变量，见第 3.1 节 |
| Actions 的 guard job 标红 | 缺 `TAURI_SIGNING_PRIVATE_KEY` secret → 按报错指引添加后重跑 |
| Release 里没有 `latest.json` | 确认正式发布（非 draft/prerelease）；确认 `createUpdaterArtifacts: true` 与 updater 配置齐全 |
| 客户端永远显示“已是最新” | 当前版本 ≥ GitHub 最新；或 Release 仍是 draft/prerelease |
| 客户端检查更新失败 | 网络不通 GitHub；代理环境可在 `src/api/updater.ts` 的 check 加 `proxy` |
| 旧平台交叉编译失败 | Tauri 依赖本机 WebView，需在该系统上构建（用 CI 矩阵） |
| macOS 跨机器打不开 | 未做 Apple 签名：右键→打开→系统设置允许 |

---

## 11. 相关文件索引

| 用途 | 文件 |
|---|---|
| 一键发版脚本（版本回写 + tag） | `scripts/release.mjs` |
| 发布门禁（fmt/clippy/test/build） | `scripts/verify.mjs` |
| CI 构建发布工作流 | `.github/workflows/publish.yml` |
| updater 端点 / 公钥 / 更新产物 | `src-tauri/tauri.conf.json` |
| 插件注册 | `src-tauri/src/lib.rs` |
| 权限声明 | `src-tauri/capabilities/default.json` |
| 更新 API 封装 | `src/api/updater.ts` |
| 设置页「软件更新」UI | `src/views/SettingsView.vue` |
| 启动静默更新提示 | `src/App.vue` |
