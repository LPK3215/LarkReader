# LarkReader 认证机制分析文档

> **本文档记录 lark-cli 的完整认证流程、token 存储机制、以及 LarkReader 需要如何封装。**
> 基于本机实际验证（2026-09-05），所有信息来自真实运行环境。

---

## 一、认证全景图

lark-cli 的认证分 **两步**，两步都走飞书 OAuth Device Flow（设备授权流程）：

```
步骤 1: config init   → 创建/绑定飞书应用 → 获得 appId + appSecret
步骤 2: auth login    → 用户浏览器扫码登录 → 获得 user token（含 scope 权限）
```

两步完成后，lark-cli 就能以用户身份调用飞书 API。token 会自动缓存，后续调用自动刷新。

---

## 二、认证流程详解

### 2.1 步骤 1：创建飞书应用（config init）

**命令**：
```bash
lark-cli config init --new --brand feishu --lang zh
```

**发生了什么**：
1. lark-cli 调用飞书开放平台 API `/oauth/v1/app/registration`，创建一个新的飞书应用
2. 飞书返回一个浏览器链接，用户在浏览器中完成应用创建/绑定
3. 创建成功后，lark-cli 获得 `appId` 和 `appSecret`

**本机实际结果**：
- appId: `cli_aa11bcce79b8dcbc`
- appSecret: 存储在系统密钥链（keychain）中，不在配置文件里明文保存

**日志证据**（来自 `~/.lark-cli/logs/auth-*.log`）：
```
path=/oauth/v1/app/registration status=200  → 注册成功
```

**判断是否已完成**：
```bash
lark-cli config show
# 返回 JSON 且包含 appId 则已完成
```

### 2.2 步骤 2：用户登录授权（auth login）

**命令**：
```bash
lark-cli auth login --domain docs --domain drive --domain wiki
```

**发生了什么**：
1. lark-cli 调用飞书 `/oauth/v1/device_authorization` 接口，发起设备授权
2. 飞书返回一个验证 URL：`https://accounts.feishu.cn/oauth/v1/device/verify?...`
3. 用户在浏览器中打开该 URL → 扫码登录飞书 → 勾选授权范围
4. lark-cli 轮询飞书 `/open-apis/authen/v2/oauth/token` 接口（每 5 秒一次），直到用户完成授权
5. 授权成功后，飞书返回 user token（access_token + refresh_token + scope 列表）

**日志证据**：
```
path=/oauth/v1/device_authorization status=200  → 设备授权发起成功
path=/open-apis/authen/v2/oauth/token status=400  → 轮询中（用户还没完成）
path=/open-apis/authen/v2/oauth/token status=400  → 轮询中
...（每 5 秒一次，共 5 次 400）
path=/open-apis/authen/v2/oauth/token status=200  → 用户完成授权，拿到 token
```

**本机实际结果**：
- 用户：`用户614265`（openId: `ou_2c7935fa3acadbfde7be2410ff7cc359`）
- 授权的 scope（部分关键权限）：
  - `docs:document.content:read` — 读取文档内容
  - `docs:document.media:download` — 下载文档媒体文件
  - `wiki:node:read` / `wiki:node:retrieve` — 读取 Wiki 节点
  - `wiki:space:read` — 读取知识库空间
  - `drive:file:download` — 下载文件
  - `offline_access` — 离线访问（支持 token 刷新）

**token 有效期**：
- access_token：2 小时（`expiresAt` 距 `grantedAt` 差 2 小时）
- refresh_token：7 天（`refreshExpiresAt` 距 `grantedAt` 差 7 天）
- 过期后 lark-cli **自动刷新**，不需要重新登录

**判断是否已完成**：
```bash
lark-cli whoami
# tokenStatus: "ready" 或 "needs_refresh" 都算已登录
# "needs_refresh" 表示 access_token 过期了，但下次调用会自动刷新
```

### 2.3 步骤 3（可选）：非阻塞模式登录

lark-cli 的 `auth login` 支持非阻塞模式，适合 GUI 应用：

```bash
# 第 1 步：发起设备授权，立即返回，不等待
lark-cli auth login --domain docs --domain drive --domain wiki --no-wait --json
# 返回: { "device_code": "xxx", "verification_url": "https://..." }

# 第 2 步：用户在浏览器中完成授权（应用打开浏览器）

# 第 3 步：用 device_code 完成登录
lark-cli auth login --device-code "xxx"
# 返回授权结果
```

> **LarkReader 应使用非阻塞模式**：Rust 后端调 `--no-wait` 拿到 URL → 通过 Tauri 事件推送给前端 → 前端打开浏览器 → 用户完成后 → Rust 后端调 `--device-code` 完成登录。

---

## 三、Token 存储机制

### 3.1 文件结构

```
~/.lark-cli/                          # 根目录（Windows: C:\Users\<user>\.lark-cli\）
├── config.json                       # 应用配置（appId、brand、用户列表）
├── update-state.json                 # 更新状态
├── cache/
│   └── remote_meta.meta.json         # 远程元数据缓存
├── hermes/                           # Agent 环境相关（可忽略）
├── locks/
│   └── refresh_<appId>_<openId>.lock  # token 刷新锁（防并发刷新）
└── logs/
    └── auth-YYYY-MM-DD.log           # 认证日志
```

### 3.2 config.json 内容

```json
{
  "apps": [
    {
      "appId": "cli_aa11bcce79b8dcbc",
      "appSecret": {
        "source": "keychain",                    // ← 敏感数据存在系统密钥链
        "id": "appsecret:cli_aa11bcce79b8dcbc"   // ← 密钥链中的标识
      },
      "brand": "feishu",
      "lang": "zh_cn",
      "users": [
        {
          "userOpenId": "ou_2c7935fa3acadbfde7be2410ff7cc359",
          "userName": "用户614265"
        }
      ]
    }
  ]
}
```

**关键发现**：
- `appSecret` 不在文件中明文存储，而是存在系统密钥链（Windows Credential Manager / macOS Keychain）
- user token（access_token / refresh_token）同样由 lark-cli 内部管理，不在 config.json 中
- LarkReader **不需要自己管理 token**——lark-cli 全权负责存储、刷新、过期处理

### 3.3 LarkReader 不需要数据库

| 问题 | 答案 |
|---|---|
| LarkReader 需要自己存 token 吗？ | **不需要**。lark-cli 全权管理 |
| LarkReader 需要数据库吗？ | **不需要**。配置由 lark-cli 管理，token 在密钥链中 |
| LarkReader 需要自己刷新 token 吗？ | **不需要**。lark-cli 自动刷新，`needs_refresh` 状态下次调用自动恢复 |
| LarkReader 需要做什么？ | 调用 lark-cli 命令时传入 `--as user`，lark-cli 自动处理认证 |

---

## 四、HERMES_HOME 环境变量问题

### 4.1 问题

CatPaw 等 AI Agent 工具会设置 `HERMES_HOME` 环境变量。lark-cli 检测到这个变量后，会认为自己运行在 Agent 环境中，报错：
```
hermes context detected but lark-cli is not bound to it
```

### 4.2 解决方案

**Rust 中每次调用 lark-cli 时清除环境变量**：

```rust
use std::process::Command;

fn build_lark_command(args: &[&str]) -> Command {
    let mut cmd = Command::new("lark-cli");
    cmd.args(args);
    // 清除干扰环境变量
    cmd.env_remove("HERMES_HOME");
    cmd.env_remove("OPENCLAW_HOME");
    cmd.env_remove("LARK_CHANNEL");
    cmd
}
```

这样无论从什么环境启动 LarkReader，lark-cli 都能正常运行。

---

## 五、LarkReader 的认证封装方案

### 5.1 完整认证流程（新机器首次使用）

```
用户打开 LarkReader
  │
  ├── env.rs: 检测 Node.js
  │   └── 未安装 → 提示用户安装（提供下载链接）
  │
  ├── env.rs: 检测 lark-cli
  │   └── 未安装 → 自动执行 npm install -g @larksuite/cli
  │
  ├── env.rs: 检测飞书应用配置
  │   ├── 调用 lark-cli config show
  │   └── 未配置 → 执行 lark-cli config init --new --brand feishu --lang zh
  │       ├── 非阻塞模式，拿到验证 URL
  │       ├── 通过 Tauri 事件推送给前端 → 前端打开浏览器
  │       └── 等待用户完成应用创建
  │
  ├── env.rs: 检测用户登录状态
  │   ├── 调用 lark-cli whoami
  │   └── 未登录 → 执行 lark-cli auth login --domain docs --domain drive --domain wiki
  │       ├── 非阻塞模式（--no-wait --json），拿到验证 URL
  │       ├── 通过 Tauri 事件推送给前端 → 前端打开浏览器
  │       ├── 用户扫码登录 + 勾选授权范围
  │       └── Rust 后端用 --device-code 完成登录
  │
  └── 认证完成，进入主界面
```

### 5.2 日常使用（已配置过）

```
用户打开 LarkReader
  │
  ├── env.rs: 调用 lark-cli whoami
  │   ├── tokenStatus: "ready" → 直接进入主界面
  │   └── tokenStatus: "needs_refresh" → 直接进入主界面（首次调用 API 时自动刷新）
  │
  └── 异常情况（token 彻底过期，refresh_token 也失效）→ 引导重新登录
```

### 5.3 env.rs 需要实现的检测函数

```rust
// 检测 Node.js
fn check_node() -> Option<String>  // 返回版本号

// 检测 lark-cli
fn check_lark_cli() -> Option<String>  // 返回版本号

// 检测飞书应用配置
fn check_app_config() -> Option<AppConfig>  // 返回 appId / brand

// 检测用户登录状态
fn check_login_status() -> LoginStatus
// LoginStatus { logged_in: bool, user_name: Option<String>, token_status: String }

// 一键安装 lark-cli
async fn install_lark_cli() -> Result<()>

// 发起飞书应用配置（非阻塞）
async fn init_app_config() -> Result<VerificationUrl>

// 发起用户登录（非阻塞）
async fn start_login() -> Result<DeviceInfo>
// DeviceInfo { device_code: String, verification_url: String }

// 完成用户登录（用 device_code 轮询）
async fn complete_login(device_code: &str) -> Result<LoginResult>
```

### 5.4 lark-cli 调用封装（lark.rs）

所有 lark-cli 调用都必须经过这个封装：

```rust
pub fn run_lark(args: &[&str]) -> Result<String> {
    let output = build_lark_command(args)
        .output()
        .map_err(|e| AppError::LarkCliNotFound(e.to_string()))?;

    if !output.status.success() {
        return Err(AppError::LarkCliError(
            String::from_utf8_lossy(&output.stderr).to_string()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
```

### 5.5 健康检查（doctor）

lark-cli 自带 `doctor` 命令，一次性检查所有项：

```bash
lark-cli doctor
```

返回 9 项检查结果：
| 检查项 | 说明 |
|---|---|
| cli_version | lark-cli 版本 |
| cli_update | 是否最新版 |
| config_file | 配置文件是否存在 |
| app_resolved | 飞书应用是否已绑定 |
| bot_identity | Bot 身份是否可用 |
| user_identity | 用户身份是否可用 |
| identity_ready | 至少一个身份可用 |
| endpoint_open | 飞书开放平台是否可达 |
| endpoint_mcp | 飞书 MCP 是否可达 |

> LarkReader 的 `check_env` 可以直接调 `lark-cli doctor` + `lark-cli whoami` 组合，一步到位。

---

## 六、lark-cli 安装位置

| 平台 | 路径 |
|---|---|
| Windows | `C:\Users\<user>\AppData\Roaming\npm\lark-cli` (npm 全局) |
| macOS | `/usr/local/bin/lark-cli` 或 `~/.npm-global/bin/lark-cli` |

npm 全局包路径：`C:\Users\<user>\AppData\Roaming\npm\node_modules\@larksuite\cli\`

---

## 七、关键命令速查表

| 用途 | 命令 | 阻塞 | 输出 |
|---|---|---|---|
| 健康检查 | `lark-cli doctor` | 是 | JSON，9 项检查 |
| 当前身份 | `lark-cli whoami` | 是 | JSON，含 tokenStatus |
| 认证状态 | `lark-cli auth status` | 是 | JSON，含 scope / 过期时间 |
| 查看配置 | `lark-cli config show` | 是 | JSON，含 appId / brand |
| 创建应用 | `lark-cli config init --new --brand feishu --lang zh` | 是 | 阻塞直到用户完成 |
| 发起登录（非阻塞） | `lark-cli auth login --domain docs --domain drive --domain wiki --no-wait --json` | 否 | JSON，含 device_code + URL |
| 完成登录 | `lark-cli auth login --device-code <code>` | 是 | 阻塞直到完成 |
| 列出已登录用户 | `lark-cli auth list` | 是 | JSON |
| 检查 scope | `lark-cli auth check --scope docs:document.content:read` | 是 | JSON |
| 退出登录 | `lark-cli auth logout` | 是 | 清除 token |

---

## 八、auth login 的非阻塞模式（重点）

### 8.1 为什么用非阻塞模式

Tauri 应用是 GUI 程序，不能阻塞主线程。`auth login` 默认会阻塞直到用户完成浏览器授权，这在 GUI 中是不可接受的。

### 8.2 非阻塞模式流程

```
Rust 后端                              前端
    │                                    │
    ├── lark-cli auth login --no-wait ──→│
    │   返回: { device_code, url }       │
    │                                    │
    │   ←── Tauri event: "login_url" ───→│ 打开浏览器
    │                                    │   用户扫码登录
    │                                    │   勾选授权范围
    │                                    │
    │   （轮询或等待用户确认）             │
    │                                    │
    ├── lark-cli auth login              │
    │   --device-code <code>             │
    │   返回: 授权结果                    │
    │                                    │
    │   ←── Tauri event: "login_success"─→│ 显示成功
    │                                    │
```

### 8.3 备选方案：阻塞模式 + 后台线程

如果非阻塞模式有问题，可以用 tokio 的 `spawn_blocking` 在后台线程运行阻塞式 `auth login`，通过 Tauri 事件向前端推送日志。

---

## 九、总结

| 问题 | 答案 |
|---|---|
| LarkReader 需要自己实现 OAuth 吗？ | **不需要**，lark-cli 全权负责 |
| LarkReader 需要数据库存 token 吗？ | **不需要**，lark-cli 存在 `~/.lark-cli/` + 系统密钥链 |
| token 过期了怎么办？ | lark-cli **自动刷新**，LarkReader 无感知 |
| 新机器怎么配置？ | 检测 → 安装 lark-cli → config init → auth login，一键引导 |
| HERMES_HOME 干扰怎么办？ | Rust 的 `Command::env_remove()` 清除 |
| 用户操作是什么？ | 打开浏览器 → 扫码登录 → 勾选权限 → 完成 |
