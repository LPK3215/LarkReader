# 文档可视化资产生成脚本

本目录存放 README 与文档中引用的 SVG 资产的**生成脚本**（纯 Python 3 标准库，无第三方依赖）。
SVG 产物输出到 [`docs/assets/`](../assets/)。

## 脚本一览

| 脚本 | 产物 | 用途 |
|---|---|---|
| `generate_banner.py` | `docs/assets/banner.svg` | README 顶部品牌横幅：项目名、定位、版本徽章、E2E 实测数据面板 |
| `generate_architecture.py` | `docs/assets/architecture.svg` | 四层架构图：前端 / Tauri IPC / Rust 后端 / 外部依赖与产出 |
| `generate_export_pipeline.py` | `docs/assets/export-pipeline.svg` | 导出流水线图：URL → 检测 → 遍历 → 四类节点分流 → 事务落盘 → 统一结果 |
| `generate_e2e_coverage.py` | `docs/assets/e2e-coverage.svg` | E2E 覆盖统计：指标卡 + 扩展名分布条形图 |

## 运行方式

在仓库根目录（或任意位置，脚本会自动定位仓库根）：

```powershell
python docs/scripts/generate_banner.py
python docs/scripts/generate_architecture.py
python docs/scripts/generate_export_pipeline.py
python docs/scripts/generate_e2e_coverage.py
```

## 维护约定

- **数值必须与代码一致**：每个脚本头部都有 `DATA` 字典，并注释了每个数值的真实来源
  （`package.json` / `Cargo.toml` / `src-tauri/src/*.rs` / `docs/e2e-download-case/README.md`）。
  项目版本、命令数、测试数、E2E 结果变化后，改 `DATA` 再重跑即可，不要手改 SVG。
- **禁止删除脚本**：只在新需求时新增 `generate_<asset_name>.py`。
- README 中的引用一律使用相对路径：`![描述](./docs/assets/xxx.svg)`。
- 深色主题设计（底色 `#0B1220`），在 GitHub 亮/暗两种主题下均以整卡形式呈现。
