# Kolibri_Ai3D — Claude Code 專案規範

## 專案概述
Kolibri_Ai3D 是一個 Rust + egui + wgpu 桌面 3D CAD 建模軟體，具有：
- SketchUp 風格的即時 3D 互動建模（10 項核心功能齊全）
- Tekla 風格的鋼構模式
- CAD 匯入/匯出（DXF/DWG/SKP/OBJ/STL/glTF/PDF）
- AI 語意辨識與 4 層推斷引擎
- MCP Server（stdio + HTTP/SSE，支援 Claude Desktop / ChatGPT）
- PBR Cook-Torrance 渲染

## 架構

### Cargo Workspace（4 crates）
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
        ├── app.rs          # KolibriApp 主結構 + update loop (~1400 行)
        ├── editor.rs       # EditorState, Tool, DrawState, SelectionMode
        ├── viewer.rs       # ViewerState, RenderMode
        ├── overlay.rs      # 2D overlay 繪圖 (~1400 行) + ArcInfo
        ├── tools.rs        # 工具互動邏輯
        ├── panels.rs       # 右側面板 UI
        ├── renderer.rs     # wgpu PBR 渲染器（Cook-Torrance BRDF）
        ├── camera.rs       # OrbitCamera
        ├── snap.rs         # 吸附推斷（含 tangent、InferenceEngine 2.0）
        ├── inference.rs    # 推斷上下文
        ├── inference_engine.rs  # 4 層推斷引擎
        ├── icons.rs        # Heroicons 風格向量圖示
        ├── menu.rs         # 選單列
        ├── layout.rs       # 出圖模式
        ├── mcp_server.rs   # APP 內建 MCP (GUI Bridge)
        ├── file_io.rs      # 檔案讀寫 (impl KolibriApp)
        ├── builders/       # 鋼構建造器
        └── ...             # re-export shims (scene, halfedge, collision, csg, measure, dxf_io, obj_io, stl_io, gltf_io)
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

### 測試與建置
```bash
# 建置全 workspace
cargo build

# 執行 GUI APP
cargo run -p kolibri-cad

# 執行 MCP Server（stdio）
cargo run --bin kolibri-mcp-server

# 執行 MCP Server（HTTP + Dashboard）
cargo run --bin kolibri-mcp-server -- --http

# 執行 MCP 測試
cargo run --bin kolibri-mcp-test

# 測試 core crate（不需 GPU）
cargo test -p kolibri-core

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

- **24 個工具**：場景查詢、物件 CRUD、推拉/旋轉/縮放/對齊/鏡射、匯入匯出、undo/redo、shutdown
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
