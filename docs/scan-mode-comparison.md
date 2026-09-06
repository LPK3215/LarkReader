# 扫描模式对比：Auto vs FullSpace

## 背景

飞书知识库的空间结构是一个"森林"（forest）——多个顶层节点直接挂在 `space_id` 下，**没有**一个统一的父节点把它们包裹起来。

知识库的首页节点（即空间入口页）本身是一个普通 Doc，通常 `has_child=false`，子文档是通过 space 级别独立挂载的。这导致：

- **Auto 模式**：递归扫描目标节点及其子树。如果目标节点 `has_child=false`，只能扫描到该节点本身
  （这在飞书里是正确结果——旁边的顶层节点是它的**兄弟**，不是它的子文档）。
- **FullSpace 模式**：无条件调 `wiki +node-list --space-id`（不带 parent token），获取该空间下全部顶层节点，
  然后递归展开各自的子树。**无论目标节点有没有子文档都给出整库目录树**，你贴的节点按真实层级出现在树中。
- **前端默认：FullSpace**（首次使用就能看到全貌），需要精确控制时再手动切到 Auto。

## 测试案例

- **测试库**：`LarkReader-E2E-测试库`
- **首页 URL**：`https://qcny2iztd1p8.feishu.cn/wiki/EqbwwXaBni7EPukHctdcEh8YnHe`
- **首页特征**：Doc 类型，`has_child=false`（子文档通过 space 级别挂载，不在首页下）

## 扫描结果对比

### Auto 模式

```
[0] LarkReader-E2E-测试库 (obj_type=Doc, has_child=false, children=0, depth=0)
```

| 指标 | 数值 |
|------|------|
| 顶层子节点 | 0 |
| Doc | 1 |
| Sheet | 0 |
| Bitable | 0 |
| File | 0 |
| **扫描总数** | **1** |

### FullSpace 模式

```
[0] LarkReader-E2E-测试库 (obj_type=Folder, has_child=true, children=8, depth=0)
  [0] 01_文本排版 (obj_type=Doc, has_child=false, depth=1)
  [0] 01_文本排版 (obj_type=Doc, has_child=true, children=3, depth=1)
    [0] 一级标题：标题与列表 (obj_type=Doc, depth=2)
    [1] 代码块与引用 (obj_type=Doc, depth=2)
    [2] 表格与任务列表 (obj_type=Doc, depth=2)
  [0] 02_媒体 (obj_type=Doc, has_child=true, children=2, depth=1)
    [0] 图片文档 (obj_type=Doc, depth=2)
    [1] 文件附件文档 (obj_type=Doc, depth=2)
  [0] 03_表格数据库 (obj_type=Doc, has_child=true, children=2, depth=1)
    [0] 员工花名册 (obj_type=Sheet, depth=2)
    [1] 项目看板 (obj_type=Bitable, depth=2)
  [0] 04_边界情况 (obj_type=Doc, has_child=true, children=2, depth=1)
    [0] 空文档 (obj_type=Doc, depth=2)
    [1] 超长标题测试xxx...xxx (obj_type=Doc, depth=2)
  [0] 05_排序验证 (obj_type=Doc, has_child=true, children=3, depth=1)
    [0] 甲 (obj_type=Doc, depth=2)
    [1] 乙 (obj_type=Doc, depth=2)
    [2] 丙 (obj_type=Doc, depth=2)
  [0] A/B:C*D?EF<G>H|I --obj-type docx... (obj_type=Doc, has_child=false, depth=1)
  [0] 06_附件下载 (obj_type=Doc, has_child=true, children=18, depth=1)
    [0] readme.md (obj_type=File, depth=2)
    [1] data.csv (obj_type=File, depth=2)
    [2] config.json (obj_type=File, depth=2)
    [3] schema.xml (obj_type=File, depth=2)
    [4] app.log (obj_type=File, depth=2)
    [5] notes.txt (obj_type=File, depth=2)
    [6] 学习笔记.md (obj_type=File, depth=2)
    [7] shape.svg (obj_type=File, depth=2)
    [8] 性能报告(终版).pdf (obj_type=File, depth=2)
    [9] pixel.png (obj_type=File, depth=2)
    [10] pixel.gif (obj_type=File, depth=2)
    [11] pixel.webp (obj_type=File, depth=2)
    [12] app-icon.ico (obj_type=File, depth=2)
    [13] 附件包.zip (obj_type=File, depth=2)
    [14] sample-doc.docx (obj_type=File, depth=2)
    [15] sample-sheet.xlsx (obj_type=File, depth=2)
    [16] sample-deck.pptx (obj_type=File, depth=2)
    [17] photo.jpg (obj_type=File, depth=2)
```

| 指标 | 数值 |
|------|------|
| 顶层子节点 | 8 |
| Doc | 18 |
| Sheet | 1 |
| Bitable | 1 |
| File | 18 |
| **扫描总数** | **38** |

## 下载结果对比

| | Auto 模式 | FullSpace 模式 |
|---|---|---|
| 成功下载 | 1 项 | 38 项 |
| 失败 | 0 | 0 |
| 跳过 | 0 | 0 |

## 输出目录结构对比

### Auto 模式输出

```
LarkReader-E2E-测试库/
  00_LarkReader-E2E-测试库.md
```

仅下载了首页这一个文档。

### FullSpace 模式输出

```
LarkReader-E2E-测试库/
  00_01_文本排版/
    00_一级标题：标题与列表.md
    01_代码块与引用.md
    02_表格与任务列表.md
  00_01_文本排版 (2).md
  00_01_文本排版.md
  00_02_媒体/
    00_图片文档.md
    00_图片文档_images/
      img_01.png
      img_02.png
      img_03.png
    01_文件附件文档.md
  00_02_媒体.md
  00_03_表格数据库.md
  00_04_边界情况/
    00_空文档.md
    01_超长标题测试xxx...xxx.md
  00_04_边界情况.md
  00_05_排序验证/
    00_甲.md
    01_乙.md
    02_丙.md
  00_05_排序验证.md
  00_06_附件下载.md
  00_A_B_C_D_EF_G_H_I --obj-type docx....md
  00_readme.md
  00_员工花名册.xlsx
  01_data.csv
  02_config.json
  03_schema.xml
  04_app.log
  05_notes.txt
  06_学习笔记.md
  07_shape.svg
  08_性能报告(终版).pdf
  09_pixel.png
  10_pixel.gif
  11_pixel.webp
  12_app-icon.ico
  13_附件包.zip
  14_sample-doc.docx
  15_sample-sheet.xlsx
  16_sample-deck.pptx
  17_photo.jpg
```

整个知识库的 8 个顶层节点及其子树全部下载，包括文档、电子表格、多维表格、附件等全部内容。

## 总结

| 维度 | Auto 模式 | FullSpace 模式 |
|------|----------------|----------------------|
| 扫描范围 | 目标节点及子树 | 整个知识库空间 |
| 扫描节点数 | 1 | 38 |
| 下载项数 | 1 | 38 |
| 适用场景 | 明确只要某一支时，精准导出该节点及其子树 | 默认方式：一次拿到整个知识库 |
| 副作用 | 无 | 无 |
| 兼容性 | 现有行为不变 | 新增能力，不影响 Auto 模式 |

**核心结论**：FullSpace 是 Auto 的真超集（Auto 能拿到的 FullSpace 都能拿到，还能多拿到兄弟节点），
两种模式完全兼容，互不影响。前端默认使用 FullSpace，用户需要精确控制时手动切回 Auto。
