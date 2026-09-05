# 飞书 Wiki 文档批量提取工具

> **定位：最小可用版本（MVP）**
> 本目录只保留最基础的提取逻辑，确保随时能跑通、能复现。
> 后续的功能增强（并发下载、断点续传、递归遍历、编码修复等）
> 请在其他目录 fork 或新建项目进行迭代，**不要在此目录内升级**。

---

## 零、安装与配置（仅首次需要）

> 运行提取脚本前，必须先完成 lark-cli 的安装和飞书授权。
> 全过程只需做一次，之后 token 会缓存在本地，直接跑脚本即可。
> 如果脚本报 `hermes context detected` 错误，先执行 `$env:HERMES_HOME = $null` 清除环境变量。

### 方式一：一键配置脚本（推荐）

```powershell
# 在 PowerShell 中执行
powershell -ExecutionPolicy Bypass -File setup.ps1
```

脚本会自动完成以下 4 步，遇到需要浏览器操作时会暂停并提示你。

### 方式二：手动逐步执行

```powershell
# 0. 清除可能干扰的环境变量（每个新终端窗口都要执行）
$env:HERMES_HOME = $null
$env:OPENCLAW_HOME = $null

# 1. 安装 lark-cli（需要先有 Node.js 18+）
npm install -g @larksuite/cli@latest

# 2. 初始化飞书应用（会打开浏览器让你创建/绑定应用）
lark-cli config init --new --brand feishu --lang zh

# 3. 用户登录授权（会打开浏览器让你扫码登录飞书）
lark-cli auth login --domain docs --domain drive --domain wiki

# 4. 验证是否配置成功
lark-cli whoami
# 应看到 identity=user, tokenStatus=ready

# 5. 快速测试 API 是否通
lark-cli docs +fetch --doc "https://xxx.feishu.cn/wiki/<node_token>" --doc-format markdown --as user
# 应看到 ok=true
```

### 关于 HERMES_HOME 环境变量

CatPaw（或其他 AI Agent 工具）会设置 `HERMES_HOME` 环境变量，`lark-cli` 检测到后会误以为在 Agent 环境中，导致报 `hermes context detected but lark-cli is not bound to it` 错误。

**解决方法**：每次开新终端窗口时先执行：
```powershell
$env:HERMES_HOME = $null
```

或者一劳永逸地从系统环境变量中删除它（Win+S 搜索「环境变量」→编辑用户变量 →删除 `HERMES_HOME`）。

---

## 一、提取原理

整个提取过程基于飞书官方 CLI 工具 `lark-cli`，分三步完成：

### 第 1 步：解析 Wiki 节点结构
```bash
# 获取节点详情（space_id、node_token、是否有子节点）
lark-cli wiki +node-get --node-token "<wiki_url或node_token>" --as user --format json

# 列出某个节点下的所有子节点（递归遍历整棵树）
lark-cli wiki +node-list --space-id <space_id> --parent-node-token <node_token> --page-all --as user --format json
```
通过递归遍历 `has_child=true` 的节点，摸清整个知识库的完整目录树。

### 第 2 步：提取文档正文（Markdown 格式）
```bash
lark-cli docs +fetch \
  --doc "https://xxx.feishu.cn/wiki/<node_token>" \
  --doc-format markdown \
  --as user
```
- 返回 JSON，正文在 `data.document.content` 字段
- `--doc-format markdown` 保留标题层级、列表、加粗、代码块、表格等全部格式
- 代码块完整保留（含语言标识），不会丢行

### 第 3 步：下载文档内嵌图片
```bash
# ⚠️ media-download 需要导出权限，多数知识库会报 permission_denied
lark-cli docs +media-download --token <file_token> --output <path> --as user

# ✅ media-preview 无需导出权限，可正常下载图片（推荐）
lark-cli docs +media-preview --token <file_token> --output <path> --as user
```
- 从 Markdown 中正则匹配 `![描述](https://feishu.cn/file/<token>)` 提取 file_token
- 用 `media-preview` 逐张下载为本地图片文件（自动识别格式 png/jpg/webp）
- 下载后将 Markdown 中的远程 URL 替换为本地相对路径（如 `images/img_01.png`）

## 二、核心脚本：extract_generic.py

### 功能
- 批量提取指定文档列表的正文（Markdown）
- 自动检测并下载文档中的所有图片
- 自动将图片引用替换为本地相对路径
- 输出提取统计（字符数、图片数、代码块数）

### 使用方法
```bash
python3 extract_generic.py <输出目录> \
  "<node_token1>|<文档标题1>" \
  "<node_token2>|<文档标题2>" \
  ...
```

### 参数说明
- 第 1 个参数：输出目录路径（不存在会自动创建）
- 后续参数：`node_token|文档标题` 格式，用 `|` 分隔
  - `node_token`：飞书 wiki 节点的 token（URL 中 `/wiki/` 后面的部分）
  - `文档标题`：用于生成文件名（会自动过滤非法字符）

### 示例
```bash
python3 extract_generic.py ./output \
  "QJFEw6cH0iSry4kRUcMcDttfn4e|从0开始学习agent" \
  "EKc8wYUawiRjKgkn1LYc6DKGn4b|Agent技术"
```

### 输出结构
```
output/
├── 从0开始学习agent.md              # 文档正文（图片引用已替换为本地路径）
├── 从0开始学习agent_images/         # 该文档的图片目录
│   ├── img_01.png
│   ├── img_02.webp
│   └── ...
├── Agent技术.md
└── ...
```

## 三、为什么 CLI 能绕过前端的复制限制

飞书文档在浏览器里可能禁了复制、右键、选择，但这些只是**前端 UI 层的限制**。
`lark-cli` 走的是**飞书开放平台 HTTP 接口**，和浏览器前端是两条完全不同的路：

| 层 | 谁在限制 | 对 CLI 的影响 |
|---|---|---|
| 浏览器前端 | 禁复制、禁右键、禁选择 | 完全无效，CLI 不经过浏览器 |
| 开放平台 API | 只看 token 有没有对应 scope | 只要账号有阅读权限就能拿 |

**结论**：排除 API 层面的限制（频率、token 过期、企业关 OpenAPI 等）后，
只要你的飞书账号有文档的阅读权限，CLI 就能拿到正文和图片，前端没有任何手段能拦住。

这也是 `media-preview` 能绕过 `media-download` 权限墙的原因：
- `media-download` 要的是**导出权限**（文档管理员可关）
- `media-preview` 只要**阅读权限**（你能看就能下）

---

## 四、关键注意事项

1. **图片下载必须用 `media-preview`，不能用 `media-download`**
   - `media-download` 需要文档的导出权限，多数共享知识库会报 `permission_denied`
   - `media-preview` 只需读取权限，可正常下载图片

2. **图片 token 从 Markdown 中正则提取**
   - 匹配模式：`!\[([^\]]*)\]\(([^)]+)\)`
   - URL 格式：`https://feishu.cn/file/<file_token>`
   - 取 URL 最后一段作为 file_token

3. **特殊字符可能导致正则漏匹配**
   - 图片描述中含 `\[`、`\]` 等转义字符时，正则可能漏匹配
   - 解决：提取后用 `grep "feishu.cn/file" *.md` 检查是否有残留远程引用，手动补下载

4. **node_token vs obj_token**
   - `docs +fetch` 接受 wiki URL、node_token、obj_token 均可
   - 推荐用 wiki URL 格式：`https://xxx.feishu.cn/wiki/<node_token>`

5. **递归遍历知识库**
   - 先用 `wiki +node-get` 获取根节点的 space_id
   - 再用 `wiki +node-list --parent-node-token` 逐层列出子节点
   - 对 `has_child=true` 的节点继续递归，直到遍历完整棵树

## 五、环境依赖

- `lark-cli`：飞书官方命令行工具（安装方式见「第零节」）
- Node.js 18+：lark-cli 的运行时依赖
- Python 3.8+
- 网络可访问飞书开放平台 API

### 文件清单

| 文件 | 说明 |
|---|---|
| `setup.ps1` | 一键安装配置脚本（首次使用时运行） |
| `extract_generic.py` | 批量提取脚本（日常使用） |

## 六、验证提取完整性

提取完成后，建议执行以下检查：

```bash
# 1. 检查是否有残留的远程图片引用（应为0）
grep -r "feishu.cn/file" output/*.md

# 2. 统计代码块数量（偶数个才完整）
for f in output/*.md; do
  count=$(grep -c '^```' "$f")
  echo "$(basename "$f"): $((count/2)) 个代码块"
done

# 3. 验证图片文件数量与引用数量一致
for d in output/*_images; do
  echo "$(basename "$d"): $(ls "$d" | wc -l) 张图片"
done
```
