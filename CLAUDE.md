# Kolibri_Ai3D — Claude Code 專案規範

## 專案概述
Kolibri_Ai3D 是一個 Rust + egui + wgpu 桌面 3D CAD 建模軟體，具有：
- SketchUp 風格的即時 3D 互動建模
- Tekla 風格的鋼構模式
- CAD 匯入（DXF/DWG/SKP/OBJ/PDF）
- AI 語意辨識與智能推斷

## 架構原則（參考 Pascal Editor 風格）

### 三層 Store 分離
應用狀態分為三個獨立的 store，禁止跨層直接耦合：
- **SceneStore** — 場景資料（節點、幾何、材質、群組、undo/redo）
- **ViewerStore** — 視圖狀態（相機、渲染模式、grid、背景色）
- **EditorStore** — 工具狀態（當前工具、選取、snap、inference、繪圖狀態）

### 扁平節點字典
場景節點使用 `HashMap<NodeId, SceneNode>` + `parent_id: Option<NodeId>` 結構：
- O(1) 查找與更新
- 透過 `parent_id` 維護層級關係（取代巢狀結構）
- Undo/redo 記錄節點快照，不需複製整棵樹

## 開發規範

### 語言與框架
- **語言**：Rust（100%）
- **UI 框架**：egui 0.28（eframe + wgpu 後端）
- **3D 數學**：glam 0.28
- **序列化**：serde + serde_json

### 程式碼風格
- 遵循 Rust 標準命名（snake_case 函數、CamelCase 型別）
- 繁體中文註解（UI 文字與程式碼註解皆使用繁體中文）
- 保持 `pub(crate)` 可見性，除非需要跨 crate 共用
- 避免 `unwrap()`，使用 `expect("具體原因")` 或 `?` 運算子

### 檔案組織
```
app/src/
├── app.rs          # KolibriApp 主結構 + update loop
├── scene.rs        # SceneStore: 節點、材質、undo/redo
├── panels.rs       # 右側面板 UI
├── tools.rs        # 工具互動邏輯
├── icons.rs        # Heroicons 風格向量圖示
├── menu.rs         # 選單列
├── layout.rs       # 出圖模式
├── camera.rs       # OrbitCamera
├── renderer.rs     # wgpu 渲染器
├── snap.rs         # 吸附推斷
├── inference.rs    # 推斷上下文
├── inference_engine.rs  # 推斷引擎 2.0
├── halfedge.rs     # 半邊資料結構
├── collision.rs    # 碰撞偵測
├── csg.rs          # CSG 布林運算
├── file_io.rs      # 檔案讀寫
├── cad_import/     # CAD 匯入模組
├── import/         # 統一匯入管線
├── builders/       # 鋼構建造器
└── dwg_parser/     # DWG 解析器
```

### UI 設計原則
- **Figma 風格淺色玻璃擬態**主題
- 面板圓角 18px，按鈕圓角 12px
- 品牌色 `#4c8bf5`，背景 `#f5f6fa`
- 游標跟隨式數值顯示（SketchUp 風格）
- 工具選擇不切換模式（任何工具都能選取物件）
- 即時回饋（吸附提示、推斷指示、hover 高亮）

### 測試與建置
```bash
# 建置
cargo build -p kolibri_ai3d

# 執行
cargo run -p kolibri_ai3d

# 檢查
cargo clippy -p kolibri_ai3d
```

### Git 規範
- Commit 訊息使用英文，簡短描述變更目的
- 前綴：`feat:`, `fix:`, `refactor:`, `ui:`, `docs:`, `perf:`
- 每個 commit 應該是可編譯的狀態

## AI Agent 操作邊界

### 允許
- 修改 `app/src/` 下的 Rust 原始碼
- 新增模組（需同步更新 `main.rs` 的 `mod` 宣告）
- 修改 `Cargo.toml` 依賴
- 建立/修改文件檔案

### 需確認
- 刪除現有模組或大規模重構
- 變更 wgpu 渲染管線
- 修改檔案格式（影響向下相容性）

### 禁止
- 修改 `.git/` 目錄
- 執行 `cargo publish`
- 在未經確認下推送到遠端
