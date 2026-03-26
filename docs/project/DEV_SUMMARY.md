# Kolibri_Ai3D 開發總結

> 更新日期：2026-03-26
> 25,681 行 Rust，60 模組

---

## 專案概況

| 項目 | 值 |
|------|---|
| 語言 | Rust 2021 |
| 渲染 | wgpu + WGSL shader |
| UI | egui (eframe 0.28) |
| 平台 | Windows 11 |
| 架構 | 三層 Store 分離（Pascal Editor 風格） |

---

## 架構設計

應用狀態分三層 Store，禁止跨層直接耦合：

| Store | 結構 | 職責 |
|-------|------|------|
| **SceneStore** | `scene: Scene` | 節點 `HashMap<id, SceneObject>` + parent_id 層級、材質、undo/redo |
| **ViewerState** | `viewer: ViewerState` | 相機、渲染模式、背景色、顯示設定、Layout |
| **EditorState** | `editor: EditorState` | 工具、選取、snap、inference、鋼構模式 |

---

## 核心模組

```
src/
├── app.rs          (2705)   KolibriApp + ViewerState + EditorState + 主題
├── tools.rs        (2999)   工具互動 + 點擊/拖曳處理
├── panels.rs       (1832)   UI 面板（toolbar + 右側 + 場景）
├── renderer.rs     (1675)   wgpu 渲染管線 + shader
├── scene.rs         (696)   SceneStore: 節點字典 + undo/redo + 層級查詢
├── snap.rs          (634)   推斷/捕捉系統
├── inference_engine.rs (584) Inference 2.0 評分管線
├── icons.rs         (541)   22 個 Heroicons 風格向量圖標
├── collision.rs     (508)   碰撞偵測
├── layout.rs        (457)   出圖模式
├── halfedge.rs      (396)   半邊網格資料結構
├── camera.rs        (188)   軌道相機 + 行走 + 正交投影
├── cad_import/             CAD 語意解析（DXF 軸線/柱梁/標高）
├── import/                 統一匯入管線（DWG/SKP/PDF/OBJ）
├── dwg_parser/             DWG 二進制解析器（R2000–R2025）
└── builders/               鋼構建造器
```

---

## 功能概覽

- **22 個繪圖工具**：選取/移動/旋轉/縮放/線/弧/矩形/圓/方塊/圓柱/球/推拉/偏移/跟隨/量測/油漆桶/標註 等
- **6 種渲染模式**：著色/線框/X 光/隱藏線/單色/草稿
- **29 種建築材質**：程序化紋理（磚/木/金屬/混凝土/大理石/磁磚/柏油/草地）
- **鋼構模式**：Grid/Column/Beam/Brace/Plate/Connection + H 型鋼斷面
- **CAD 匯入**：DXF/DWG/SKP/OBJ/PDF + 語意偵測 + 確認面板
- **AI 整合**：MCP Server（11 工具）+ AI 審計日誌 + 語意建模助手
- **匯出**：OBJ/STL/GLTF/DXF + PNG/JPG 截圖
- **50 步 Undo/Redo** + 自動儲存 + 最近檔案

---

## 近期里程碑

### 2026-03-26：架構重構
- 三層 Store 分離（SceneStore / ViewerState / EditorState）
- 扁平節點字典（SceneObject + parent_id）
- CLAUDE.md 專案規範

### 2026-03-24：CAD 匯入 + 鋼構
- DXF/DWG/PDF 匯入系統 + 語意偵測器
- Snap 系統改為 3D 螢幕空間
- 鋼構模式 6 工具 + 碰撞偵測
- 標註/文字工具 + 陣列複製

### 2026-03-23：核心功能
- 從零建立可用的 SketchUp-like 3D CAD 應用
- Figma 風格 UI + 22 個工具 + 推斷/捕捉
- MCP Server + AI 審計日誌

---

## 文件索引

| 文件 | 說明 |
|------|------|
| [CLAUDE.md](../../CLAUDE.md) | AI Agent 專案規範 |
| [ROADMAP.md](ROADMAP.md) | 功能清單 + 待辦追蹤 |
| [IMPORT_DESIGN.md](IMPORT_DESIGN.md) | CAD 匯入架構設計 |
| [PLUGIN_AND_MCP.md](PLUGIN_AND_MCP.md) | 外掛系統 + MCP 連接 |
| [kolibri_ux_upgrade_plan.md](kolibri_ux_upgrade_plan.md) | UX 升級計畫 |
| [LEFT_PANEL_STEEL_TOOLS.md](LEFT_PANEL_STEEL_TOOLS.md) | 鋼構模式規劃 |
| [rust_module_scoring_engine_ui_flow.md](rust_module_scoring_engine_ui_flow.md) | Inference 2.0 設計 |

---

## 快速啟動

```bash
# 開發建置
cd app && cargo run

# Release 建置
cargo build --release

# MCP Server 模式
target/release/kolibri-cad.exe --mcp
```

桌面捷徑：`C:\Users\localadmin\Desktop\Kolibri CAD.lnk`
