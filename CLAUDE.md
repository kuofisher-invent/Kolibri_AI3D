# Kolibri_Ai3D — Claude Code 專案規範

## 專案概述
Kolibri_Ai3D 是一個 Rust + egui + wgpu 桌面 3D/2D CAD 建模軟體，具有：
- SketchUp 風格的即時 3D 互動建模（推拉/旋轉/縮放/偏移 等完整工具）
- Tekla 風格的鋼構模式（CNS 386 H 型鋼規格，feature gate）
- 管線繪製模式（PVC/EMT/消防鐵管/不鏽鋼/銅管，CNS 標準，feature gate）
- BIM/IFC 屬性（IfcColumn/IfcBeam/IfcWall/IfcPipeSegment）
- CAD 匯入/匯出（DXF/DWG/SKP/OBJ/STL/glTF/PDF）
- AI 語意辨識與 4 層推斷引擎
- MCP Server（stdio + HTTP/SSE，支援 Claude Desktop / ChatGPT）
- PBR Cook-Torrance 渲染（雙面渲染、背面藍灰色調）
- 面板顯示/隱藏 Toggle（工具列/屬性/Console）
- ZWCAD 風格 2D 出圖模式（深色 Ribbon 41 工具 + SVG icons + 2D Canvas + 點格線 + 十字游標）

## 架構

### Cargo Workspace（7 crates）
```
Kolibri_Ai3D/
├── Cargo.toml              # workspace root
├── crates/
│   ├── core/               # 純邏輯核心（無 GUI 依賴）
│   │   ├── scene.rs        # Scene, SceneObject, Shape, MaterialKind
│   │   ├── halfedge.rs     # HeMesh 半邊資料結構
│   │   ├── collision.rs    # 碰撞偵測
│   │   ├── command.rs      # Command Pattern Undo/Redo (Diff + Full)
│   │   ├── csg.rs          # CSG 布林運算
│   │   ├── dimensions.rs   # 標註資料型別
│   │   ├── geometry.rs     # GeometryKernel trait
│   │   ├── error.rs        # thiserror 錯誤型別
│   │   └── measure.rs      # 測量工具
│   ├── io/                 # 檔案匯入/匯出
│   │   ├── dxf_io.rs       # DXF (LINE/3DFACE/CIRCLE 匯入, 完整匯出)
│   │   ├── obj_io.rs       # OBJ + .mtl 材質匯入/匯出
│   │   ├── stl_io.rs       # STL 二進位匯入/匯出（含 Mesh）
│   │   ├── gltf_io.rs      # glTF 匯出
│   │   ├── cad_import/     # DXF 智慧匯入管線
│   │   ├── import/         # 統一匯入管線
│   │   └── dwg_parser/     # DWG 解析器
│   ├── skp/                # SketchUp SDK FFI（匯入+匯出）
│   │   ├── ffi.rs          # 動態載入 SketchUpAPI.dll
│   │   ├── converter.rs    # SKP → SkpScene 轉換
│   │   └── exporter.rs     # Scene → .skp 匯出
│   ├── piping/             # 管線外掛（feature: piping）
│   │   ├── pipe_data.rs    # PipeSystem, PipeSegment, PipeFitting
│   │   ├── catalog.rs      # CNS 標準管材規格目錄
│   │   ├── geometry.rs     # 圓柱 Mesh 管段/彎頭/三通/閥門
│   │   └── tools.rs        # PipingTool, PipingState
│   ├── drafting/           # 2D 出圖引擎（feature: drafting）
│   │   ├── entities.rs     # DraftEntity, DraftDocument, DraftObject
│   │   ├── layer.rs        # DraftLayer, LayerManager
│   │   └── geometry.rs     # offset, trim, mirror, array, rotate
│   └── mcp/                # MCP Server（4 層架構）
│       ├── protocol.rs     # JSON-RPC 2.0 協定型別
│       ├── adapter.rs      # 17 tools → Scene API 轉接
│       ├── dashboard.rs    # Web Dashboard（內嵌 HTML/JS/CSS）
│       ├── transport_stdio.rs  # Claude Desktop stdio
│       ├── transport_http.rs   # ChatGPT HTTP/SSE (axum)
│       └── bin/
│           ├── server.rs       # kolibri-mcp-server 執行檔
│           └── test_harness.rs # 自動化測試
└── app/                    # GUI 應用程式
    └── src/
        ├── app/            # KolibriApp 主結構（模組化）
        │   ├── mod.rs      # KolibriApp struct + new()
        │   ├── update.rs   # update loop + keyboard/viewport dispatch
        │   ├── update_ui.rs # draw_panels (topbar, toolbar, right panel, status)
        │   └── commands.rs # 指令調度
        ├── editor.rs       # EditorState, Tool, DrawState, SelectionMode, WorkMode
        ├── viewer.rs       # ViewerState, RenderMode, show_grid/axes/toolbar/right_panel
        ├── overlay/        # 2D overlay 繪圖
        │   ├── cursor.rs   # 游標提示卡、snap 圓點、ghost line
        │   ├── gizmo.rs    # 選取框、Move gizmo、Scale handles
        │   ├── hud.rs      # 樓層指示、軸向、比例尺、toasts
        │   ├── guides.rs   # 材質選取器、推拉參考線、量角器
        │   └── navigation.rs # 視角按鈕、nav pad、面板 toggle
        ├── tools/          # 工具互動邏輯
        │   ├── viewport.rs # 拖曳/滾輪/PushPull move handler
        │   ├── click.rs    # on_click 分發
        │   ├── click_draw.rs   # 繪圖工具狀態機
        │   ├── click_edit.rs   # 編輯工具（Move/Rotate/Scale/PushPull/Steel）
        │   ├── keyboard.rs     # 快捷鍵（含 Mirror/Flip）
        │   ├── measure.rs      # VCB 尺寸輸入（含單位解析 m/cm/ft/'/"）
        │   ├── menu_actions.rs # 選單動作（匯出/CSG/Hide/Explode/SKP export）
        │   ├── geometry_ops.rs # H型鋼 CNS 規格表 + mesh 生成 + Mirror/Flip
        │   └── picking.rs      # pick() + pick_face()（slab method）
        ├── panels/         # UI 面板
        │   ├── toolbar.rs  # 左側工具列（模式下拉 + 工具按鈕）
        │   ├── ribbon.rs   # ZWCAD 深色 Ribbon（4 tab, 41 工具, SVG icons）
        │   ├── draft_canvas.rs  # 2D 深色畫布 + 點格線 + 十字游標 + 互動
        │   ├── tab_properties.rs # 右側屬性面板（DIMENSIONS/TRANSFORM/BIM-IFC/MATERIAL）
        │   ├── tab_scene.rs     # 場景面板 + status_text()
        │   ├── tab_help.rs      # 說明面板（操作/快捷鍵/管線規格/鋼構規格/法規）
        │   └── material_swatches.rs # 材質色票
        ├── renderer/       # wgpu PBR 渲染器
        │   ├── pipeline.rs # ViewportRenderer + dirty-flag caching
        │   ├── mesh_builder.rs # 場景 mesh 建構（含 face highlight）
        │   ├── primitives.rs   # push_box/cylinder/sphere
        │   ├── shaders.rs      # WGSL shader（雙面渲染 + back-face tint）
        │   └── helpers.rs      # 工具函式
        ├── camera.rs       # OrbitCamera（35° FOV, zoom toward cursor）
        ├── snap.rs         # 吸附推斷（含 tangent、14 種 SnapType）
        ├── inference.rs    # 推斷上下文
        ├── inference_engine.rs  # 4 層推斷引擎（Geometry/Context/Semantic/Intent）
        ├── icons.rs        # 向量圖示（含 Steel/Piping/Walk/Section icons）
        ├── svg_icons.rs    # SVG icon loader（resvg → TextureHandle, 85 icons）
        ├── menu.rs         # 選單列（檔案/編輯/工具/檢視/版面）+ 右鍵選單
        ├── layout.rs       # 出圖模式（Paper Space）
        ├── mcp_server.rs   # APP 內建 MCP (GUI Bridge)
        └── file_io.rs      # 檔案讀寫
```

### 三層 Store 分離（Pascal Editor 風格）
- **SceneStore** (`core/scene.rs`) — 場景資料（節點、幾何、材質、群組、undo/redo）
- **ViewerStore** (`viewer.rs`) — 視圖狀態（相機、渲染模式、grid、背景色）
- **EditorStore** (`editor.rs`) — 工具狀態（當前工具、選取、snap、inference）

### 扁平節點字典
`HashMap<NodeId, SceneNode>` + `parent_id: Option<NodeId>`：
- O(1) 查找與更新
- Undo/redo 使用 Command Pattern（Diff + Full 混合堆疊）

## 開發規範

### 語言與框架
- **語言**：Rust（100%）
- **UI 框架**：egui 0.28（eframe + wgpu 後端）
- **3D 數學**：glam 0.28
- **序列化**：serde + serde_json
- **錯誤處理**：thiserror（core）
- **日誌**：tracing + tracing-subscriber
- **HTTP**：axum + tokio（MCP server）

### 程式碼風格
- 遵循 Rust 標準命名（snake_case 函數、CamelCase 型別）
- 繁體中文註解（UI 文字與程式碼註解皆使用繁體中文）
- core/io crate 使用 `pub` 可見性；app crate 使用 `pub(crate)`
- 避免 `unwrap()`，使用 `?` 或 `unwrap_or`

### Feature Flags
```toml
[features]
default = ["steel", "piping", "drafting"]
steel = []                                    # 鋼構模式（CNS 386 H 型鋼）
piping = ["dep:kolibri-piping"]               # 管線模式（CNS 管材規格）
drafting = ["dep:kolibri-drafting", "dep:resvg"] # 2D 出圖模式（ZWCAD Ribbon + SVG icons）
```

### 測試與建置
```bash
# 建置全 workspace（含所有 feature）
cargo build

# 建置 release
cargo build --release

# 只建模模式（不含鋼構/管線）
cargo build -p kolibri-cad --no-default-features

# 執行 GUI APP
cargo run -p kolibri-cad

# 測試管線幾何
cargo test -p kolibri-piping

# 測試 core crate（不需 GPU）
cargo test -p kolibri-core

# 執行 MCP Server（HTTP + Dashboard）
cargo run --bin kolibri-mcp-server -- --http

# 檢查
cargo clippy
```

### Git 規範
- Commit 訊息使用英文，簡短描述變更目的
- 前綴：`feat:`, `fix:`, `refactor:`, `ui:`, `docs:`, `perf:`
- 每個 commit 應該是可編譯的狀態

### UI 設計原則
- **Figma 風格淺色玻璃擬態**主題
- 面板圓角 18px，按鈕圓角 12px
- 品牌色 `#4c8bf5`，背景 `#f5f6fa`
- 游標跟隨式數值顯示（SketchUp 風格）
- Crossing/Window 選取（左→右 藍框、右→左 綠虛框）
- 即時回饋（吸附提示、推斷指示、hover 高亮、move gizmo）

## MCP Server

詳見 [docs/MCP_SERVER.md](docs/MCP_SERVER.md)。

- **26 個工具**：場景查詢、物件 CRUD、推拉/旋轉/縮放/對齊/鏡射、牆/板建立、匯入匯出、undo/redo、shutdown
- **MCP Resources**：`kolibri://scene` 即時場景 JSON
- **MCP Prompts**：3 個建築預設（simple_building, column_grid, room_layout）
- **4 層架構**：protocol → adapter → transport (stdio/HTTP) → test
- **Web Dashboard**：`http://localhost:3001/`
- **APP 內建按鈕**：頂部列 MCP 按鈕一鍵啟動

## AI Agent 操作邊界

### 允許
- 修改 `app/src/`、`crates/` 下的 Rust 原始碼
- 新增模組（需同步更新 `mod` 宣告）
- 修改 `Cargo.toml` 依賴
- 建立/修改文件檔案
- 透過 MCP 或 process kill 關閉 APP 以重新編譯

### 需確認
- 刪除現有模組或大規模重構
- 變更 wgpu 渲染管線
- 修改檔案格式（影響向下相容性）

### 禁止
- 修改 `.git/` 目錄
- 執行 `cargo publish`
- 在未經確認下推送到遠端
