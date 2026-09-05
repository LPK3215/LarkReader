# LarkReader 飞书登录授权与权限体系说明

> 定位：专门说明「飞书登录」与「应用权限」的全部细节——它们分几层、各是什么、层与层之间的关系、为什么有的环节不出现权限勾选、出现权限问题时怎么排查。
> 依据：本项目真实代码（`src-tauri/src/{lark.rs,env.rs,commands.rs,models.rs}`）与已发生的真实报错。代码实现始终是最终依据。

---

## 1. 一句话模型：两层凭证 + 一次用户授权

飞书的鉴权体系在 LarkReader 中拆成 **互相独立的两层凭证**，外加业务操作时逐资源的权限判断：

```
┌─────────────────────────────────────────────────────────────┐
│  ① 应用层凭证（App）        ② 用户层授权（User）             │
│  命令: config init --new    命令: auth login                 │
│  产出: app_id/app_secret    产出: 用户令牌（token）          │
│  站点: open.feishu.cn       站点: accounts.feishu.cn         │
│  =「这个 CLI 应用是谁建的」  =「应用能以谁的身份访问资源」      │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
        业务操作全部以用户身份执行         实际能否读到某篇文档/文件
        （所有命令固定 --as user）         由“三层权限”共同决定（见 §4）
```

- **① 应用层**：告诉飞书“你是谁的应用”。每个使用 LarkReader 的开发者/用户在自己的机器上跑一次 `config init --new`，创建属于自己的自建应用（形如 `cli_xxx`），凭据存在 `~/.lark-cli/config.json`。
- **② 用户层**：告诉飞书“这个应用以谁的身份取数据”。用户在本机完成一次 `auth login`，令牌由 lark-cli 托管。
- 两者**互不依赖**：配置了应用不等于已登录；已登录也不代表这台机器配置过应用。所以 UI 的环境体检把两者**并行**检测。

---

## 2. 两个流程分别做了什么、为什么不长一样

### 2.1 `config init --new` = 创建应用（开发者动作）

| 项 | 说明 |
|---|---|
| 实际命令 | `lark-cli config init --new --brand feishu --lang zh` |
| 打开页面 | `open.feishu.cn/page/cli?user_code=…`（**开放平台**） |
| 页面性质 | “开发者确认创建应用”向导 |
| 需要什么 | 扫码 = 登录开放平台的**开发者身份**，然后确认创建这个 app |
| 是否有权限勾选 | **没有**。它不把任何文档/文件数据授权给应用，只是“建一个 app” |
| 产物 | `app_id` / `app_secret`，写入本机 `~/.lark-cli/config.json`，**没有 token**（所以命令结束仍是未登录） |

> 这就是“点一下就成功、却没有让我点权限”的原因——你确认的是「创建应用」，不是「允许访问我的数据」。

### 2.2 `auth login` = 用户授权（资源动作）

| 项 | 说明 |
|---|---|
| 实际命令 | `lark-cli auth login --domain docs --domain drive --domain wiki` |
| 打开页面 | `accounts.feishu.cn/oauth/v1/device/verify`（**账号授权**） |
| 页面性质 | OAuth 设备码授权页 |
| 需要什么 | 登录飞书账号并**同意应用请求的权限范围** |
| 是否有权限勾选 | **有**。这里才会列出应用申请访问的权限类目 |
| 产物 | 用户令牌，由 lark-cli 托管；之后业务命令都以该用户身份执行 |

### 2.3 域名判据（快速区分你看到的是哪一步）

| 域名 | 归属 | 会出现权限勾选吗 |
|---|---|---|
| `open.feishu.cn/…` | 开发者后台 / 创建应用 / 配置权限 | 不会（不含数据授权） |
| `accounts.feishu.cn/…` | 用户授权页（OAuth） | 会 |

---

## 3. 设备码登录流程（本项目实现细节）

完整时序（`src-tauri` 侧）：

1. 前端调 `start_login` → 后端执行
   `auth login --domain docs --domain drive --domain wiki --no-wait --json`
2. 解析返回的 `{ device_code, verification_url }`，把 URL 交给前端**打开系统浏览器**。
   后端代码中的兜底默认 URL：`https://accounts.feishu.cn/oauth/v1/device/verify`。
3. 用户在浏览器登录飞书账号 → 看到授权页 → 同意。
4. 前端拿 `device_code` 调 `complete_login` → 后端执行
   `auth login --device-code <code>`，**单次阻塞**等待（最长约 10 分钟，后端超时上限 620s，略大于 lark-cli 内部上限，避免临界误杀）。
5. 后端再跑 `whoami` 校验：`identity == "user"` 且 `token_status ∈ {ready, needs_refresh}` → 判定登录成功。

### 3.1 本项目踩过的坑（重要，勿重蹈）

| 坑 | 后果 | 对策 |
|---|---|---|
| 用 3 秒轮询反复查登录状态 | 每重启一次 `auth login` 就作废上一轮 device code，永远授权不成功 | 改为**单次阻塞等待**授权（提交 ef6e7bd） |
| 并发发起两个 `auth login --device-code` | 后一个会作废前一个的 code | 串行化，禁止并发/重启 |
| 把阻塞式命令放进 IPC 串行队列 | 卡死其他调用 | 登录放独立线程 |
| 工具链里残留 `HERMES_HOME` / `OPENCLAW_HOME` / `LARK_CHANNEL` 环境变量 | 污染 lark-cli 行为（本项目曾在沙箱环境被注入） | 每次构造子进程时 `env_remove` 这三个变量 |

---

## 4. 权限体系：三层，缺一不可

业务命令（如读取某篇 wiki 文档）能否成功，由**三层独立条件**同时满足决定：

```
第 1 层  应用后台 scope（第 ① 层凭证 + 开发者后台配置）
         该自建应用是否开通了对应的 OpenAPI 权限点
         （新建应用 = 全空，必须回开放平台补！）

第 2 层  用户授权（第 ② 层凭证）
         用户是否在 accounts.feishu.cn 授权页点了同意
         （对应登录时的 --domain docs/drive/wiki 申请）

第 3 层  资源本身对该用户可见
         文档/文件是否属于该用户，或已被分享给该用户
```

> **最常见的“授权成功但仍报无权限”根因**：第 1 层没配。新建的自建应用（`cli_xxx`）后台权限点是空的，即使你完成了登录授权，请求对应数据接口仍会被飞书拒绝。对策：到开放平台该应用后台开通对应权限点，然后重新授权（新 token 才带新 scope）。

### 4.1 本项目三个登录域 `--domain` 对应什么业务

| 域 | 用途（本项目实际使用的命令） | 备注 |
|---|---|---|
| `docs` | 文档正文：`docs +fetch --as user`（Markdown）；图片/媒体：`docs +media-preview` | 飞书文档正文、图片 |
| `drive` | 云盘文件：`drive +preview --type source_file --as user`（下载 Wiki 挂载的普通附件 zip/pdf/…） | 不能用 `drive +download` 拿非可导出文件（见 §6） |
| `wiki` | 知识库结构：`wiki +node-get` / `wiki +node-list --page-all --as user` | 目录遍历、判断节点类型 |

需要如实说明的点：

- 登录时**只申请以上三个域**（代码写死 `["docs","drive","wiki"]`）。
- Sheet 导出（`sheets +workbook-export`）与多维表格导出（`base +record-list`）**不在**登录单独声明的域清单里——它们复用同一个用户令牌请求数据，能否成功仍以开放平台对该应用开通的权限点为准。
- `domain` 是登录授权页上的“大类目”，不等于 OpenAPI 细粒度权限点；细粒度权限在开放平台应用后台管理。

---

## 5. 登录状态与判定（UI 显示的依据）

`whoami` 返回字段（后端 `WhoamiResponse`）：

| 字段 | 取值 | 含义 |
|---|---|---|
| `identity` | `user` / `bot` / 空 | 本应用业务全部走 **`user`**（所有命令 `--as user`） |
| `tokenStatus` | `ready` / `needs_refresh` / `none` | 令牌可用 / 需要刷新（仍视为有效）/ 未登录 |
| `onBehalfOf.userName` | 用户名 | 头像、欢迎语展示 |

环境体检 `check_env` 并行检测两项并给出四个独立状态：

| 状态 | 来源 | 含义 |
|---|---|---|
| `app_configured` | `config show` 成功 | 本机已配置飞书应用 |
| `logged_in` | `whoami`：user 且 ready/needs_refresh | 用户已授权登录 |
| `token_status` | `whoami` | none = 未登录 |
| `check_errors` | 检测过程异常 | 分别报告，不互相掩盖 |

登出：`auth logout --json` → 清除 lark-cli 保存的令牌 → 再次体检即回到未登录。登出/切号前应提示用户：**后续任务需要重新授权**。

---

## 6. 本项目实证过的真实报错与处理（排查手册）

| 报错/现象 | 原因 | 本项目处理 |
|---|---|---|
| `current identity does not have export permission for this Drive file` | `drive +download` 对 zip/pdf 等非可导出类型不适用 | 改用 `drive +preview --type source_file` 直接取原文件（代码注释明示） |
| 授权页不弹 / device code 失效 | 轮询或并发重启了 `auth login` | 单次阻塞 + 串行（§3.1） |
| `unsafe output path` | lark-cli 1.0.93 写类命令有输出路径白名单 | 把子进程 cwd 设为输出目录所在目录，使该目录成为白名单内当前目录 |
| 授权成功但业务请求仍无权限 | 新建应用后台 scope 为空（§4 第 1 层） | 回开放平台补权限点 → 重新授权 |
| 输出夹带日志行导致 JSON 解析失败 | 命令 stdout 可能混入日志 | 统一先 `extract_json` 再解析 |

---

## 7. 安全边界（本项目刻意不做的事）

- 后端**不保存飞书密码**。
- 后端**不自行维护 token**：令牌由 lark-cli 统一托管。
- 令牌只在本机用于代表用户身份访问飞书，**不通过网络发送给任何第三方**；导出只是把用户有权限看到的文档原样落盘到用户本地目录。
- 每次调用 lark-cli 都清除可能注入的工具环境变量（HERMES_HOME 等）。
- 后端对输出路径做白名单/真实可写校验与词法展开，避免路径穿越与 Windows 路径不一致问题。

---

## 8. 快查对照表

| 你看到的页面 | 域名 | 是什么 | 要不要点权限 |
|---|---|---|---|
| CLI 二维码/链接 + “应用配置成功” | `open.feishu.cn/page/cli` | 创建自建应用 | 否 |
| “请打开链接完成授权 / 设备码” | `accounts.feishu.cn` | 用户 OAuth 授权 | 是 |
| 环境卡显示“未配置应用” | — | 本机没跑过 `config init` | — |
| 环境卡显示“未登录” | — | 本机没跑过 `auth login` | — |

相关命令速记：

| 目标 | 终端命令 / UI 入口 |
|---|---|
| 创建应用（每机一次） | UI「一键自动创建」= 后台 `config init --new` |
| 登录授权 | UI「开始登录」= `auth login --domain docs --domain drive --domain wiki` |
| 查状态 | 环境体检 = `config show` + `whoami` |
| 退出登录 | UI「退出登录」= `auth logout` |
