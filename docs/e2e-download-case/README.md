# e2e-download-case —— E2E 全量下载案例

本目录是从真实飞书知识库下载得到的**完整下载案例**，用于直观展示 LarkReader 后端
下载链路对各种飞书节点类型、文件格式与目录规则的实际还原效果。

**线上原地址（一键打开）**：<https://qcny2iztd1p8.feishu.cn/wiki/EqbwwXaBni7EPukHctdcEh8YnHe?from=from_copylink>

## 案例来源

| 项 | 值 |
|---|---|
| 知识库 | LarkReader-E2E-测试库（E2E 测试样例库） |
| 线上原地址 | [在飞书打开本案例](https://qcny2iztd1p8.feishu.cn/wiki/EqbwwXaBni7EPukHctdcEh8YnHe?from=from_copylink) |
| space_id | `7681927602327538869` |
| 顶层节点 | 8 个（本目录即其全量下载产物） |
| 下载方式 | `wiki::extract_wiki` 对每个顶层节点 URL 递归全量导出 |
| 下载时间 | 2026-09-05 |
| 结果 | **38 项成功 / 0 失败 / 0 跳过**（38 = 18 个 docx 正文 + 20 个特殊导出：Sheet 1 + Bitable 1 + 文件附件 18） |

> 说明：飞书知识库的一个 space 下可以有多个互不相连的"根节点"，本测试库共有 8 个，
> 因此全量下载 = 对 8 个顶层节点逐一递归导出。测试库内容是为验证而造的样例，
> 结构稳定，可反复下载复现。

### 8 个顶层节点（下载入口）

各节点下载 URL = `https://feishu.cn/wiki/<node_token>`（06 附件容器亦可用
`https://qcny2iztd1p8.feishu.cn/wiki/Iwhuw7jDbiwvm1kodW6cCpZVnsb`）：

| # | 标题 | node_token |
|---|---|---|
| 1 | 01_文本排版（单篇，无子） | `LuQ7wEqgmiqITJkL49zcjyA0nif` |
| 2 | 01_文本排版（父，3 子文档） | `EAMOwgdxZiuJcGk7SggcXFGnnXf` |
| 3 | 02_媒体（图片/附件文档） | `GALuwGcesiqpOXkFIKYccYtbnZo` |
| 4 | 03_表格数据库（Sheet/Bitable） | `KpGawWRGtif1PGkdAbscQBGZntd` |
| 5 | 04_边界情况（空/超长标题） | `LhydwRkCdil1oNkn1eHccMrhnDf` |
| 6 | 05_排序验证（甲/乙/丙） | `WggMwupWaiRueskwSTtc0J00nId` |
| 7 | 特殊字符标题文档 | `CnMowYMTOiHj3pkWa66cHqfRnne` |
| 8 | 06_附件下载（18 文件附件） | `Iwhuw7jDbiwvm1kodW6cCpZVnsb` |

## 内容规模统计

- 总文件数：**42**
- 子目录数：**10**
- 总大小：约 **1.5 MB**（1,538 KB）

### 扩展名分布（18 种）

| 扩展名 | 数量 | 说明 |
|---|---|---|
| `.md` | 20 | 18 个 docx 正文导出的 Markdown + 2 个 md 格式文件附件（00_readme.md、06_学习笔记.md） |
| `.png` | 4 | 3 张正文图片（02_媒体/图片文档）+ 1 个附件 |
| `.json` | 2 | 附件 config.json + Bitable 元数据 manifest |
| `.xlsx` | 2 | Sheet 导出（员工花名册）1 + 附件 1 |
| `.csv` / `.xml` / `.log` / `.txt` / `.svg` / `.pdf` / `.jpg` / `.gif` / `.webp` / `.ico` / `.docx` / `.pptx` / `.zip` | 各 1 | 全部为 06_附件下载 下的文件附件 |
| `.ndjson` | 1 | Bitable 记录导出（项目看板） |

## 覆盖的节点类型与场景

本案例一次覆盖 LarkReader 支持的全部下载路径：

| 节点类型 | 呈现 | 所在位置 |
|---|---|---|
| docx 正文（普通文档） | 编号前缀 + 标题命名 `.md` | 各目录下 `NN_xxx.md` |
| 父文档 + 子页面树 | 父文档正文与后代全部下载，子页面落入父文档目录 | `01_文本排版 (2)/`（父+3 子）、`05_排序验证/`（父+甲乙丙） |
| 图片正文 | `.md` 内图片 URL 本地化为相对路径，图片落 `NN_标题_images/` | `02_媒体/00_图片文档.md` + `00_图片文档_images/img_0{1,2,3}.png` |
| 单篇顶层文档（无子） | 独占一个顶层目录 | `01_文本排版/`、`A_B_C.../` |
| 空文档 | 忠实还原（内容为空则正文为空） | `04_边界情况/00_空文档.md` |
| 超长标题 | 完整保留 | `04_边界情况/01_超长标题…（约80字）.md` |
| 特殊字符文件名 | 非法文件名字符清洗为 `_` | `A_B_C_D_EF_G_H_I --obj-type….md`（含 `/:*?"<>|` 等） |
| Sheet（电子表格） | 导出 `.xlsx` | `03_表格数据库/00_员工花名册.xlsx` |
| Bitable（多维表） | 记录导出 `.ndjson` + 元数据 `.manifest.json` | `03_表格数据库/01_项目看板/` |
| 文件附件（file 节点） | 原样字节下载，文件名=层内前缀+原标题+扩展名 | `06_附件下载/` 下 18 个附件 |

### 附件专项：06_附件下载

容器页面本身也是 docx（正文可下载），其下挂 **18 个文件附件、17 种扩展名**，
用于验证"附件字节级往返"与"文件名还原"：

```
00_readme.md          07_shape.svg        13_附件包.zip
01_data.csv           08_性能报告(终版).pdf  14_sample-doc.docx
02_config.json        09_pixel.png        15_sample-sheet.xlsx
03_schema.xml         10_pixel.gif        16_sample-deck.pptx
04_app.log            11_pixel.webp       17_photo.jpg
05_notes.txt          12_app-icon.ico
06_学习笔记.md
```

覆盖：文本类（md/csv/json/xml/log/txt）、图像类（svg/pdf/png/gif/webp/jpg）、
二进制（ico/zip）、Office 三件套（docx/xlsx/pptx）、中文文件名（学习笔记.md）、
含括号文件名（性能报告(终版).pdf）。

## 目录结构（全量）

```
e2e-download-case/
├── 01_文本排版/                      单文档页（1 篇）
├── 01_文本排版 (2)/                  父文档 + 3 个子文档（4 篇）
│   ├── 00_01_文本排版.md
│   ├── 00_一级标题：标题与列表.md
│   ├── 01_代码块与引用.md
│   └── 02_表格与任务列表.md
├── 02_媒体/                          3 篇文档 + 图片落盘
│   ├── 00_02_媒体.md
│   ├── 00_图片文档.md
│   ├── 00_图片文档_images/           img_01.png / img_02.png / img_03.png
│   └── 01_文件附件文档.md
├── 03_表格数据库/
│   ├── 00_03_表格数据库.md
│   ├── 00_员工花名册.xlsx            Sheet 导出
│   └── 01_项目看板/
│       ├── 01_Table.manifest.json    Bitable 元数据
│       └── 01_Table.ndjson           Bitable 记录导出
├── 04_边界情况/                      3 篇（含空文档、超长标题）
├── 05_排序验证/                      父 + 甲/乙/丙（4 篇，验证顺序）
├── 06_附件下载/                      容器正文 + 18 个文件附件
└── A_B_C_D_EF_G_H_I --obj-type…/     特殊字符标题清洗后的文档
```

## 命名与结构规则（从本案例可观察）

- **编号前缀** = 节点在其父节点内的**树序**（`00_` 开头为目录/文档自身正文或首个对象）
- **重名目录** 自动追加 ` (2)` 区分
- **子页面跟随父文档**：父文档下挂子页面时，整棵下载、子页面进父文档目录
- **图片**：正文同目录旁生成 `<标题>_images/`，md 内为相对路径引用，可纯本地渲染
- **特殊字符** `\ / : * ? " < > |` 在文件名中清洗为 `_`
- **特殊节点**按自身类型导出（docx→md、Sheet→xlsx、Bitable→ndjson、file→原文件字节）

## 与仓库内其他目录的关系

| 目录 | 定位 |
|---|---|
| `docs/e2e-fixtures/` | **素材来源**：当初往测试库上传的原材料（正文样例 md、图片/音频/pdf、表数据 json、gen_assets.py、tokens.json） |
| `e2e_download_tmp/` | **测试工作目录**：回归跑批临时输出，每次测试前清空 |
| `docs/e2e-download-case/`（本目录） | **下载结果案例**：E2E 测试库全量下载的稳定快照，长期留档 |

> 本目录是使用 **FullSpace 扫描模式**手动全量下载后长期留档的稳定快照。
> 现存真实下载自动化回归见 `tests/z_tmp_big_download.rs`（针对“实测大型知识库（A）”，
> 可 `cd src-tauri && cargo test --test z_tmp_big_download` 复跑）。

## 扫描模式说明

本案例使用 **FullSpace 扫描模式**导出整库。LarkReader 内置两种扫描模式：

- **Auto 模式**：仅导出目标节点及其子树。适合已知"目标节点就是一个含子文档的父节点"的场景。
- **FullSpace 模式**：当目标节点 `has_child=false` 时自动 fallback 到 `wiki +node-list --space-id` 抓取空间全部顶层节点，再递归各自的子树。适合"知识库首页本身只是个入口页、子文档都挂在 space 级别"的场景——本案例就属于这一种。

本案例 Auto 模式仅能扫到 1 项（首页正文），FullSpace 模式扫到 38 项（整库）。两种模式完全兼容，按需切换。原理与差异详见 [docs/scan-mode-comparison.md](../scan-mode-comparison.md)。
