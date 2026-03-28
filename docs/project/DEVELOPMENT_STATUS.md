# Kolibri_Ai3D 開發進度與規劃

> 最後更新：2026-03-29

## 專案概述

Kolibri_Ai3D 是一款 **Rust + egui + wgpu** 桌面 3D CAD 建模軟體，目標對標 SketchUp 的互動體驗，同時提供 Tekla 風格的鋼構建模功能。

---

## 一、已完成功能

### 1. 核心建模（10 項 SketchUp 核心功能）

| # | 功能 | 說明 |
|---|------|------|
| 1 | **Line 工具** | 點擊畫線，吸附推斷，角度鎖定 |
| 2 | **Rectangle 工具** | 兩點矩形，即時尺寸顯示 |
| 3 | **Circle 工具** | 圓形/多邊形，分段數可調 |
| 4 | **Arc 工具** | 三點弧，tangent 推斷 |
| 5 | **Push/Pull** | 面推拉擠出 3D 實體 |
| 6 | **Move** | 拖曳移動，軸鎖定，Ctrl 複製，即時渲染更新 |
| 7 | **Rotate** | 任意軸旋轉，角度吸附 15° |
| 8 | **Scale** | 均勻/非均勻縮放，面中心=單軸 |
| 9 | **Offset** | 面偏移（外擴/內縮） |
| 10 | **Eraser** | 拖曳連續刪除 |

### 2. 推斷引擎（4 層 InferenceEngine 2.0）

| 層級 | 功能 |
|------|------|
| L1 | 軸平行（紅/綠/藍）|
| L2 | 端點/中點/邊上吸附 |
| L3 | 面法線平行/垂直 |
| L4 | Tangent（弧線切點）|

### 3. 渲染

- **Clay 風格渲染**（2026-03-29 更新）
  - 明亮 ambient（0.45 + hemisphere blend）
  - 柔和 diffuse（0.45 倍率）
  - 低 specular（×0.3，消除刺眼高光）
  - 柔和陰影（最暗 0.6）
  - 邊線柔灰 `[0.35]`，粗度預設 1.0
- **PBR Cook-Torrance BRDF** 渲染管線
- **Section Plane** — GPU fragment shader discard
- **Ground shadow** — 方向性投影陰影
- 渲染模式：Shaded / Wireframe / X-Ray / Sketch
- 背面剔除（backface culling）

### 4. 選取與互動

- **Crossing/Window 選取** — 左→右藍框（Window）、右→左綠虛框（Crossing）
- **選取高亮** — 45% 原色 + 55% 藍，AABB 包圍框，dirty check 即時更新
- **Move Gizmo** — XYZ 三軸箭頭
- **游標跟隨式數值顯示**（SketchUp 風格）
- **Zoom** — +/- 按鈕 + 滾輪 zoom_toward（游標中心縮放），範圍 10mm～200m
- **視角切換動畫** — ease-out cubic 0.3s

### 5. 場景管理

- **扁平節點字典** — `HashMap<NodeId, SceneNode>` O(1) 查找
- **Command Pattern Undo/Redo** — Diff + Full 混合堆疊
- **碰撞偵測** — Move 時即時碰撞警告
- **群組/元件** — 雙擊編輯，嵌套支援

### 6. MCP Server（AI 語意控制）

- **26 個工具** — 場景 CRUD、推拉/旋轉/縮放、牆/板/柱建立、匯入匯出
- **雙傳輸** — stdio（Claude Desktop）+ HTTP/SSE（ChatGPT）
- **Web Dashboard** — `http://localhost:3001/`
- **APP 內建 MCP** — 一鍵啟動，HTTP bridge 控制 GUI 場景
- **MCP Resources** — `kolibri://scene` 即時場景 JSON
- **MCP Prompts** — 3 個建築預設

### 7. 檔案匯入/匯出

| 格式 | 匯入 | 匯出 | 說明 |
|------|------|------|------|
| **SKP** | ✅ SDK 原生 | — | SketchUp C SDK v14.1，子進程隔離 |
| **DXF** | ✅ LINE/3DFACE/CIRCLE | ✅ 完整 | 智慧匯入管線 |
| **DWG** | ✅ 基礎解析 | — | 自研解析器 |
| **OBJ** | ✅ + .mtl 材質 | ✅ | UV 支援 |
| **STL** | ✅ 二進位 | ✅ | Mesh 匯入 |
| **glTF** | — | ✅ | per-object materials |
| **PDF** | ✅ 基礎 | — | 向量提取 |

### 8. SKP SDK 匯入

- **SketchUp C API v14.1** 動態載入（全球首個 Rust wrapper）
- **35+ FFI 函數綁定**（含 SUModelCreateFromFileWithStatus、SUFaceGetFrontMaterial、SUFaceGetEdges、SUEdgeToDrawingElement、SUDrawingElementGetHidden）
- `SUMeshHelper` 正確三角化（含凹多邊形）
- **座標修正** — negate-X 保持手性 `SU(X,Y,Z) → Kolibri(-X,Z,Y)`，inch→mm ×25.4
- **邊線渲染** — `SUFaceGetEdges` 讀取面輪廓邊（官方範例推薦方式），soft/smooth/hidden 三重過濾，跟 SketchUp 顯示完全一致
- **Entities 快取** — 避免重複 SDK 呼叫導致 crash（同一 component/group def 只 SDK 讀取一次）
- **子進程隔離** — `kolibri-skp-worker`，DLL crash 不影響主 APP
- **Fallback** — SDK 失敗自動退回 bridge/heuristic
- **版本預檢** — 從檔案 header 偵測 SKP 版本
- **GPU buffer 保護** — 截斷 > 160MB mesh，防止 wgpu panic
- 測試結果：component_sample.skp → **73 meshes, 37K vertices** — 文字方向正確、邊線乾淨

### 9. UI/UX

- **Figma 風格淺色玻璃擬態主題**
- 品牌色 `#4c8bf5`
- 面板圓角 18px，按鈕圓角 12px
- 格線自適應密度
- Undo/Redo 步數顯示
- 右鍵選單（反轉面等）
- 頂部工具列 + 右側屬性面板
- **匯入後自動置中**（AABB → center XZ, bottom Y=0）

### 10. 鋼構模式

- 柱/樑/板/牆建造器
- 柱網生成
- 碰撞規則（梁不可穿柱等）

---

## 二、程式碼品質（2026-03-29）

### 檔案拆分（全部 < 1000 行）

| 原檔案 | 行數 | 拆成 | 子模組 |
|--------|------|------|--------|
| `tools.rs` | 3706 | 9 files | viewport, keyboard, click, click_draw, click_edit, measure, picking, menu_actions, geometry_ops |
| `app.rs` | 2717 | 6 files | mod, update, update_ui, import_tasks, commands, mcp_handler |
| `panels.rs` | 2258 | 4 files | material_swatches, toolbar, tab_properties, tab_scene |
| `renderer.rs` | 2150 | 6 files | shaders, pipeline, mesh_builder, primitives, helpers, mod |
| `overlay.rs` | 2045 | 6 files | cursor, guides, gizmo, navigation, hud, mod |
| `dxf_importer.rs` ×2 | 1561 | 5 files | types, parser, entity_parsers, tests, mod |

最大檔案：`scene.rs` 948 行。

### SKP SDK 穩定化

- 移除 14 個 debug eprintln（converter.rs）
- 移除 3 個 dead functions
- 移除 unused struct fields
- 修正 unsafe unwrap → match fallback
- 加 bounds check on array access

---

## 三、已知問題

| # | 問題 | 嚴重度 | 說明 |
|---|------|--------|------|
| 1 | 大場景 SKP 匯入記憶體高 | 中 | 709 meshes → ~4GB RAM，需增量 mesh 建構 |
| 2 | MCP HTTP 超時 | 低 | 大場景匯入時 HTTP 連線超時（import 仍在進行） |
| 3 | MCP port 佔用 | 低 | `AddrInUse` panic，需 retry 或 graceful handling |
| 4 | SKP 材質顏色未讀取 | 低 | SUFaceGetFrontMaterial 已綁定但未使用 |

---

## 四、未來規劃

### P1：效能優化（短期）

| # | 項目 | 說明 |
|---|------|------|
| 1 | **增量 build_scene_mesh** | per-object vertex/index cache，避免每幀重建全場景 |
| 2 | **rayon 平行 mesh** | tessellation 多核加速 |
| 3 | **LOD / 視錐剔除** | 遠處物件簡化，不在畫面內的不渲染 |

### P2：渲染進化（中期）

| # | 項目 | 說明 |
|---|------|------|
| 4 | **GPU texture binding** | per-material wgpu texture bind group |
| 5 | **Shadow Map** | depth texture + 第二 render pass |
| 6 | **SSAO** | 螢幕空間環境遮蔽 |
| 7 | **真正的 UV mapping** | per-face UV 控制 |

### P3：專業功能（中長期）

| # | 項目 | 說明 |
|---|------|------|
| 8 | **SKP 匯出** | 用 SDK `SUModelCreate` + `SUModelSaveToFile` |
| 9 | **IFC 匯入/匯出** | 建築資訊模型交換 |
| 10 | **標註系統** | 尺寸標註、文字標註、引線 |
| 11 | **出圖模式** | 2D 平面圖/立面圖/剖面圖輸出 |
| 12 | **多用戶協作** | 即時同步編輯 |

### P4：kolibri-skp crate 發佈（待定）

| # | 項目 | 說明 |
|---|------|------|
| 13 | **API 文件** | 完整 rustdoc |
| 14 | **安全 Drop** | 解決 SUModelRelease crash |
| 15 | **材質讀取** | SUFaceGetFrontMaterial + 色彩提取 |
| 16 | **UV 座標** | SUMeshHelper STQ coords |
| 17 | **crates.io 發佈** | 全球首個 Rust SketchUp SDK wrapper |

### P5：架構債（低緊迫）

| # | 項目 | 說明 |
|---|------|------|
| 18 | **io crate 完全切換** | 移除 app 側 import/ 副本 |
| 19 | **GeometryKernel trait** | 可插拔幾何核心 |
| 20 | **Plugin 系統** | WASM/Lua 腳本擴展 |

---

## 五、技術架構

```
Kolibri_Ai3D/
├── crates/
│   ├── core/       純邏輯核心（Scene, HeMesh, Command, Collision, CSG）
│   ├── io/         檔案匯入/匯出（DXF, OBJ, STL, glTF, DWG）
│   ├── mcp/        MCP Server（stdio + HTTP/SSE, 26 tools）
│   └── skp/        SketchUp SDK FFI（動態載入, 子進程隔離）
└── app/            GUI 應用程式（egui + wgpu）
    └── src/
        ├── app/            主結構（mod, update, update_ui, import_tasks, commands, mcp_handler）
        ├── tools/          工具互動（viewport, keyboard, click*, measure, picking, menu_actions, geometry_ops）
        ├── panels/         UI 面板（material_swatches, toolbar, tab_properties, tab_scene）
        ├── overlay/        2D overlay（cursor, guides, gizmo, navigation, hud）
        ├── renderer/       wgpu 渲染器（shaders, pipeline, mesh_builder, primitives, helpers）
        ├── editor.rs       工具狀態
        ├── viewer.rs       視圖狀態
        ├── snap.rs         吸附推斷
        ├── inference_engine.rs  4 層推斷
        ├── camera.rs       OrbitCamera
        └── import/         匯入管線
```

### 三層 Store 分離

- **SceneStore** — 場景資料（節點、幾何、材質、undo/redo）
- **ViewerStore** — 視圖狀態（相機、渲染模式、grid）
- **EditorStore** — 工具狀態（當前工具、選取、snap）

### 依賴

- **語言**：100% Rust
- **UI**：egui 0.28 + eframe + wgpu
- **3D 數學**：glam 0.28
- **序列化**：serde + serde_json
- **HTTP**：axum + tokio
- **SDK**：SketchUp C API v14.1（動態載入 libloading）
