# 极端内容压测记录（写入端 → 导出端）

> 目的：用 lark-cli 的**写入端**在测试知识库造一批"刁钻到不合理"的文档（超长文本、符号/公式地狱、
> 多层嵌套、超链接地狱、超大体积），再用 **LarkReader 导出端**逐篇导出，把"写入内容"与"导出结果"
> 逐项对拍，重点看公式、符号、图片、嵌套。
> 测试库：`qcny2iztd1p8.feishu.cn`；父节点 `03_表格数据库`（wiki `KpGawWRGtif1PGkdAbscQBGZntd`）。
> 全部测试数据**保留在线上**，未清理。

---

## 一、本轮新建的压测对象

| 对象 | 类型 | 链接 |
|---|---|---|
| 压测A · 符号公式排版地狱 | docx | <https://qcny2iztd1p8.feishu.cn/wiki/L0pJwZVL2iIzmrk4LB2c7h7jnJg> |
| 压测B · 嵌套地狱 | docx | <https://qcny2iztd1p8.feishu.cn/wiki/AxgHwtm1IicbCNkGqDrc69fvntg> |
| 压测B-子文档1 · 层级嵌套 | docx | <https://qcny2iztd1p8.feishu.cn/wiki/RP30wusCkiuaUzkYWMmcMrQ4n8c> |
| 压测B-子文档2 · 更深层级 | docx | <https://qcny2iztd1p8.feishu.cn/docx/Vyn3dxBkroByqExJXSAcwEQGnKd> |
| 压测C · 超链接地狱 | docx | <https://qcny2iztd1p8.feishu.cn/docx/Gz7sdDndgoiYWmxGtE3cn6vKnsd> |
| 压测D · 大体积与宽表 | docx | <https://qcny2iztd1p8.feishu.cn/docx/Jp3ZdHvOmoduAYx7wfNcqidtnWf> |
| 压测_复杂公式表（2 子表） | sheet | <https://qcny2iztd1p8.feishu.cn/sheets/LC4nsMAYmhnGz9tvpjPcjNgJnDg> |
| 压测_大表与宽表（301 行 + 30 列） | sheet | <https://qcny2iztd1p8.feishu.cn/sheets/BFN6sUThOh1A2Etf1T9c1XBKnYe> |

写入方式：`lark-cli docs +create --doc-format xml`（DocxXML：`<latex>`/`<a>`/`<img>`/`<table>`/`<sheet>`/
`<whiteboard>`/`<source>`/`<cite>`/`<sub-page-list>`/`<html5-block>`/`<callout>`/`<grid>`/`<bookmark>`/`<button>`），
表格内容用 `lark-cli sheets +csv-put / +cells-merge / +dropdown-set / +cells-set-image / +cells-set-style`。

导出结果（修复后重跑）：**6 篇全部 `Success`、`errors: []`、零失败、零跳过**。

---

## 二、逐项对拍结论（✅ 修复后状态）

### A · 符号 / 公式 / 排版

**✅ 保留（正确）**
- 标题 `#`~`######`；行内样式（粗/斜/下划线/删除线/行内代码）+ 嵌套组合。
- 行内代码里的 `**` `_` `| a | b |` `# 不是标题` 未被误解析成 Markdown（逃逸正确）。
- 引用、分割线、带 `lang`/`caption` 的代码块、待办 `- [x]` / `- [ ]`。
- **行内公式 `<latex>` → `$...$`，以 LaTeX 源码存活**：`\frac{}{}`、`\sum_{i=1}^{n}`、`\int_{0}^{\infty}`、
  矩阵 `\begin{pmatrix}…\end{pmatrix}`、分段 `\begin{cases}…\end{cases}`、上下标 `x$^2$`、希腊字母与关系符全部在。
- Markdown 元字符转义：`\*` `\_` `` \` `` `\~` `\#` `\$` 正确。
- emoji / 肤色修饰 / ZWJ 组合 / 旗帜 / 键帽 / 组合音标（é、한글、天城文）/ RTL 混排 / 零宽字符 / BOM 全保留。
- 超长无空格串、超长单行段落完整无截断。
- 图片 2/2 落盘（`_images/img_01.png`、`img_02.png`），caption 作为 alt 保留。

**✅ 已修复**
- **`h7`/`h8`/`h9`（缺口 5）**：不再全部塌成无法区分的 `######`。导出端比对正文 6 级标题序列与 XML
  `<h7>`/`<h8>`/`<h9>` 序列，仅把"超出 6 级"的那几行改写为 `###### [h7] 2.6 更深标题 h7` 这类带层级标记的
  6 级标题；序列对不齐时整体放弃改写、保持原样（不误伤）。
- **文档级 `<table>`（缺口 6）**：`<colgroup>`/`<thead>`/`<tbody>` 已转成**管道表**，`colspan`/`rowspan`
  正确网格化（`| 跨列 |  |`、`|  | $E=mc^2$ |`）。单元格内 `<b>`→`**`、`<code>`→`` ` ``、`<em>`→`*`、
  `<a>`→`[文字](url)`、`<latex>`→`$…$`、`<img>`→`![caption](本地路径)`，管道符 `|` 转义为 `\|`。
  解析不出表格结构时保持原文并告警（不静默丢内容）。

**⚠️ 上游行为（导出端不含该逻辑，原样带出）**
- 转义轻微过度：正文里 `\\` 被上游 markdown 转换输出为 `\\\`、`\"` 输出为 `\\"`。导出端无任何转义改写
  逻辑，该现象来自 `docs +fetch --doc-format markdown` 的输出，按原样保存。
- `<source>` 附件 → 只留 `<source name=… token=…/>` 占位，**附件不落盘**。
- `<callout>` / `<grid>` / `<column>` → 原样保留 XML + 文末"未能离线还原"提示（标签：callout、grid、column、source）。

### B · 嵌套地狱

**✅ 保留**
- 内嵌表格内联成管道表，且带**计算后的值**（`=B2*C2`→59.7、`=SUM`→5039.7、`IF`→高/低、`ROUND`→3.14、
  `CONCATENATE`→A-B、emoji 拼接、`REPT` 超长串）。
- 内嵌表**下拉**：xlsx 内 `dataValidation sqref="J2:J6"` 保留 ✅。
- 内嵌表**单元格图片**：xlsx 内 `xl/media/image1.jpeg` + `<drawing>` 保留 ✅。
- 内嵌表**合并**：`mergeCell ref="A20:D20"` 保留 ✅（inline 按合并只留首格文本）。
- 内嵌表**公式本体**：xlsx 内 `<f t="shared">…</f>` 保留。
- 段落行内图 + 块级图：下载 2/2 + `![alt](_images/img_NN.png)`。
- **whiteboard(mermaid) → ```mermaid 代码块** ✅（内容保留，类型变形）。
- `<cite type="doc">` @文档 → 保留 `doc-id` + 真实 `title`。
- `<sub-page-list/>` → 展开为真实子文档 `<sub-page doc-id=… title="压测B-子文档1 · 层级嵌套"/>` ✅。

**✅ 已修复**
- **重复渲染（缺口 1）**：同一文档嵌入同工作簿的多个子表时，不再"每个块把整份工作簿渲染一遍"。
  导出端按块上的 `sheet-id` 只渲染对应子表，并优先显示子表名；实测 docB 两块分别输出 `rGdYD3`（20×8）
  与 `wj4JPM`（3×3），docD 两块分别输出 `IjLFfG` 与 `4bz2yQ`，块与子表一一对应。
- **xlsx 数量（缺口 1）**：同工作簿的多个内嵌块**只落一份** `内嵌表格_01.xlsx`（复用 token，避免重复导出）；
  不同工作簿时各自独立序号。
- **表格单元格图片（缺口 2）**：扫描 HTML 表内 `<img src="token">`，下载并落盘为
  `img_cell_<token>.png`，正文改写为 `![表格单元格里的图](…_images/img_cell_….png)`。实测
  `img_cell_Hj6NbIaY1os5vOxR5FxccKZmnUg.png` 真实落盘。
- **`<html5-block>`（缺口 3）**：从 `docs +fetch` 的 `reference_map["html5-block"]` 按 `data-ref` 取回正文，
  以 ```html 围栏代码块完整还原（含 `<h3>`/`<p>`/`<ul>`/emoji）。

**⚠️ 上游行为 / 已替代**
- `<figure><source>` 附件、行内附件 → 只留占位；文末提示（标签：figure、source、cite）。
- **跨表公式在内嵌副本里变 `#REF!`（缺口 4）**：线上原表算得 88，文档内嵌副本被飞书算成 `#REF!`。
  属**飞书侧内嵌副本行为**，导出端如实带出，不做粉饰。

### C · 超链接地狱

**✅ 保留**
- 全部渲染为 `[文字](url)`；query 参数、`#锚点`、中文/unicode 路径、`mailto:`、超长 URL 均正确还原。
- 链接文字里的 `*` 已转义；表格内链接正常。
- wiki 互链 / docx 互链 / sheets 链接 / bitable 链接全部保留。

**✅ 已修复**
- `<bookmark>` / `<button>` / `<cite type="citation">` 之前**只留 XML 且无提示**（与 A/B 不一致）。
  现统一文末提示：`（标签：bookmark、button、cite）`。为修复它，`report_leftover_tags` 抽成独立函数，
  在"无富对象引用"的提前返回分支与正常结尾各调用一次——否则这类"一行两个标签"的 `<bookmark …></bookmark>`
  根本不会进扫描路径。

**⚠️ 上游行为**
- `url-preview` 预览型链接 → 退化成普通链接（卡片形态丢失，链接本身在）。
- 裸 URL / 裸邮箱 → 保持纯文本，不自动链接（符合预期）。

### D · 大体积边界

- 301 行内嵌表：inline 只内联前 200 行，并附提示「（该子表共 301 行，此处只内联前 200 行，完整数据见下方 xlsx）」✅
  （`MAX_INLINE_TABLE_ROWS = 200`）。
- 30 列宽表：**✅ 已修复**（本轮新增）——不再"补齐到 40 列空列"。导出端裁剪尾部全空列后输出恰好 **6 行 × 30 列**；
  301 行表由网格 20 列裁为实际的 **301 行 × 8 列**若文本占满则不动。中间非空列、以及仅由图片标记撑着的列不会被裁。
- 超大单行段落：完整无截断。
- 同样命中 B 的重复渲染 / 单份 xlsx 问题（均已修复）。

### 导出外壳（共性）

- **wiki 链接** → 标题/文件名 = 真实标题（如 `压测A · 符号公式排版地狱.md`）✅。
- **docx 直链标题（缺口 8）**：**✅ 已修复**——标题解析链改为 `知识库节点标题 → 正文首行 <title> → token 兜底`。
  实测 docx 直链导出标题为真名：`压测B-子文档2 · 更深层级`、`压测C · 超链接地狱`（不再退化成 token 前 20 字符）。
- 导出的 xlsx `<dimension ref="A1">` 可能缺失/错误 → excelize 类读取器会把**有数据**的表判成"空"（数据其实都在）。
  读取 xlsx 时请放宽"空表"判定；此为第三方工具解析差异，非数据缺失。

---

## 三、缺口处理结果

| # | 现象 | 结果 | 说明 |
|---|---|---|---|
| 1 | 同文档嵌入同工作簿多个子表 → 每个块渲染整份工作簿 | ✅ 已修 | 按块 `sheet_id` 只渲染对应子表；同工作簿只落一份 xlsx |
| 2 | 表格单元格内图片不下载、不落盘 | ✅ 已修 | 扫描表内 `<img src=token>` 下载落盘并改写为本地图片 |
| 3 | `<html5-block>` 正文不落盘 | ✅ 已修 | 读 `reference_map` 按 `data-ref` 取回，以 ```html 围栏还原 |
| 4 | 跨表公式在内嵌副本里变 `#REF!` | ⛔ 不可修 | 飞书侧内嵌副本行为，导出端如实带出 |
| 5 | `h7~h9` 塌成 `######` | ✅ 已修 | 改写为 `###### [h7] …` 保留层级信息；序列对不齐则不改写 |
| 6 | 含合并/图片的文档级表格回退成裸 HTML | ✅ 已修 | HTML `<table>` → 管道表（colspan/rowspan/行内标签/latex/链接/图片） |
| 7 | xlsx 公式无 `<v>` 缓存值 | 🟡 已有替代 | 可切"纯数据模式"换取值（代价：无公式/样式/合并） |
| 8 | docx 直链导出标题退化为 token | ✅ 已修 | 标题链补正文 `<title>` 兜底 |
| 9 | `<source>`/`<figure>`/`<callout>`/`<grid>` 只留占位或原样 XML | ✅ 已修（提示统一） | 统一文末"残留标签"提示，C 的 `bookmark`/`button`/`cite` 现也提示 |

---

## 四、本轮修复明细（代码层）

| 模块 | 改动 |
|---|---|
| `markdown.rs` | 新增 `html_table_to_markdown`（HTML 表 → 管道表，含 colspan/rowspan 网格化、行内标签与 latex 转换、图片回调）；`RichRef` 加 `data_ref`、`RichRefKind::Html5`；新增 `wrap_fence`（围栏长度自适应内容中的连续反引号）、`unresolved_tags`（真标签判定：带属性或存在同名闭合才算，跳过围栏代码/常见 HTML/`<https://…>`/子页面标签）、`extract_document_title`、`has_six_level_heading`、`fix_deep_headings`（比对正文标题序与 XML `<h1..h9>` 序）；`csv_to_markdown_table_marked` 新增尾部全空列裁剪 |
| `extract.rs` | `docs_fetch_with_refs_controlled` 取回 `reference_map`；`EmbeddedContext` 带 `fetch`；`rich_ref_key` 加 `html5:<data-ref>` 分支；`render_embedded_ref` 加 Html5 分支（```html 围栏）；Table 分支改走 `html_table_to_markdown` + `table_cell_image_markdown`；`report_leftover_tags` 统一文末提示（含提前返回分支）；`resolve_document_title` 标题兜底；`render_embedded_sheet` 按 `sheet_id` 过滤、同工作簿单份 xlsx |
| `lark.rs` | `DocsFetch { content, reference_map }` + `html5_block(data_ref)`；`docs_fetch_with_refs_controlled` / `docs_fetch_xml_controlled` |
| `models.rs` | `FetchDocument` 加 `reference_map`（`#[serde(default)]`） |
| 单元测试 | 新增 12 个（HTML 表转换、data_ref、围栏自适应、残留标签三档、标题解码、深层标题两档、尾部空列裁剪）；总数 65 → **77 passed / 0 failed / 1 ignored** |

---

## 五、复现方式（临时脚本已删除，保留命令）

```powershell
# 写入端（示例：整篇 XML）
lark-cli docs +create --doc-format xml --content "@./docB.xml" --parent-token KpGawWRGtif1PGkdAbscQBGZntd --as user

# 写入端（示例：给子表塞公式/合并/下拉/图片）
lark-cli sheets +csv-put  --spreadsheet-token <TOKEN> --sheet-id <SID> --start-cell A1 --csv "@./formulas.csv" --as user
lark-cli sheets +cells-merge --spreadsheet-token <TOKEN> --sheet-id <SID> --range A20:D20 --as user
lark-cli sheets +dropdown-set --spreadsheet-token <TOKEN> --sheet-id <SID> --range J2:J6 --options '["待处理","进行中"]' --as user
lark-cli sheets +cells-set-image --spreadsheet-token <TOKEN> --sheet-id <SID> --range A22 --image ./local1.png --as user

# 导出端：`extract::extract_doc(url, out_dir, &Settings{ download_images:true, export_pure_data:false, .. })`
```

> 备注：压测期间飞书 OpenAPI 出现过多次 DNS/超时抖动，均通过重试恢复，非代码问题。

---

## 六、第二轮压测（扩展块 / 样式 / 下拉 / 外壳边界）

> 第一轮覆盖了符号公式、嵌套、超链接、大体积；第二轮专撞第一轮没碰过的面：扩展块
> （callout/grid/bookmark/button/time/cite/附件/画板）、文档样式（颜色/对齐/底色/合并）、
> 独立表格（下拉/数字格式/合并/公式）、导出外壳边界（非法文件名/超长标题/重名/空文档/纯图/纯表）。

### 新增压测对象

| 对象 | 链接 |
|---|---|
| 压测E · 样式与表格地狱2 | <https://qcny2iztd1p8.feishu.cn/docx/Ol8CdBMPto087hxJIE3c06cUnEc> |
| 压测F · 扩展块与富对象（重复标题一份） | <https://qcny2iztd1p8.feishu.cn/docx/NfD3dykzuo3ECdxD25rcjKufn1d> ／ <https://qcny2iztd1p8.feishu.cn/docx/YMBRdnRzZoDViHxcMyKcliEBnBb> |
| 压测H1 · 标题含非法文件名字符 | <https://qcny2iztd1p8.feishu.cn/docx/DojjdvVvao4YEgx10pycJLmHnqb> |
| 压测H2 · 空文档 | <https://qcny2iztd1p8.feishu.cn/docx/AbKqdwsizoiocwxddN3cEHTbneh> |
| 压测H3 · 纯图文档 | <https://qcny2iztd1p8.feishu.cn/docx/MnEhdDc67oDCWnxBDC7c0YRvnTh> |
| 压测H4 · 纯表文档 | <https://qcny2iztd1p8.feishu.cn/docx/VEoNddTHpojXfkxCtEBc98VtnHc> |
| 压测H5 · 172 字超长标题 | <https://qcny2iztd1p8.feishu.cn/docx/DJ1Ed1sxxo0cMIxaEoBcA2e3nNf> |
| 压测G · 表格格式与下拉 | <https://qcny2iztd1p8.feishu.cn/sheets/Yir9sf2tGhDRlDt46AkcsYiynQh> |

### 本轮发现并修复

1. **docx 直链标题再退化（缺口 8 的漏网）** —— 文档正文里**一个标题块都没有**时，飞书把文档
   标题渲染成首个 `# 一级标题` 而**不是** `<title>` 标签；`extract_document_title` 只认
   `<title>`，文件名于是又退回 "token 前 20 字符"。实测 docH1/H2/H3/H4/H5 全部中招。
   修复：取不到 `<title>` 时，把首个非空行的 `# 一级标题` 当作文档标题（有 `<title>` 时仍优先它）。
   修复后 `压测H2 · 空文档.md`、`压测H4 · 纯表文档.md`、长标题截断到 100 字等均正确。
2. **文件名安全化两处加固** —— ① 控制字符（`<title>` 可能解码出换行）此前不进 `safe_filename`，
   会生成 Windows 非法文件名，现统一换成 `_`；② 截断到 100 字时截断点可能正好落在点号/空格上，
   留下 Windows 不接受的结尾，现截断后再收一次尾。
3. **可还原的残留标签就地转 Markdown** —— `<bookmark>` / `<button>` / `<cite user|doc|citation>` 的
   数据全在属性里、不联网就能还原，之前却让导出正文留一串 XML。现转成
   `[书签](url)` / `[按钮文字](url)` / `@用户名` / `@文档标题` / `[文献标题](url)`，围栏代码块里的
   示例跳过。文末"残留标签"从 9 种降到 6 种（剩 callout、grid、column、time、figure、source）。

### 本轮实测结论（无需修复）

- **独立表格导出（xlsx）完整保留**：`mergeCell A6:D6`、2 个下拉（`dataValidation` D2:D4 / E2:E4）、
  3 条公式（`A2+B2` / `SUM(A2:A3)` / `ROUND(A2*100,1)`）、数字格式（`0.00%`、`$#,##0.00`、
  `yyyy-mm-dd`）、含 `| , " * _ #` 与换行的单元格文本。
- **同批重名文档**导出到同一目录 → 第二份自动加 ` (2)` 后缀（`压测F · 扩展块与富对象 (2).md`）。
- **含非法文件名字符的标题** → `压测H1 · 非法字符_ 反斜杠_ 冒号_ 星号__ …` 正常落盘。
- **空文档 / 纯图 / 纯表** 均 `Success`；纯图文档图片 1/1 下载成功。
- **文档样式**：`<span text-color/background-color>`、`align`、表头底色 `background-color`、
  `colspan`/`rowspan`、`vertical-align`、嵌套有序/无序列表、`checkbox` → 任务列表、`pre/code` 带
  `caption`；表格单元内 `**粗**` / `` `代码` `` / `$latex$` / 链接 / `\|` 转义均正确。
- **画板**：`whiteboard type=plantuml` → ```plantuml 围栏；`type=svg` → 服务端只给
  `<whiteboard token>`，导出端改写为"内嵌画板无法离线还原"的提示（图形内容无法文本化，属预期）。

### 未能覆盖 / 写入端限制（如实记录，非导出端缺陷）

- `<bitable token=…/>`：**create API 静默吞掉**（fetch 回来那一节是空的）→ "文档内嵌多维表格"
  无法用 CLI 造数；导出端已有 `RichRefKind::Bitable` 分支（每张表 → `.ndjson`），需 UI 手插一次补验。
- `okr` / `task` / `chat_card`：当前登录态缺 `okr:*`、`task:task:write`、`im:chat:read` 权限，无法造数。
- 表格 `隐藏行列 / 冻结 / 分组`（写入端 `sheet_ai/…/invoke_write` 一度连败，网络恢复后已补上）——
  导出的 xlsx 里 `<pane state="frozen" ySplit="1">`、H 列 `<col hidden="true">`、第 8 行
  `<row hidden="true" r="8">`、第 10~12 行 `<row outlineLevel="1">` 全部在 → **冻结 / 隐藏 / 分组均保留**。
- 单元格内嵌图片（`+cells-set-image`）→ xlsx 内 `xl/media/image1.jpeg` + `<drawing>` ✅ 保留。
- 表格 `筛选 / 条件格式`：`--properties` 要按 schema 传 `rules`，本轮没造数，待补。
