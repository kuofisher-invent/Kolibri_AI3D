# SketchUp C SDK 原生匯入方案 — 技術文件

## 概述

Kolibri_Ai3D 使用 SketchUp C SDK（`SketchUpAPI.dll`）直接解析 `.skp` 檔案，不需要啟動 SketchUp 應用程式。透過 `libloading` crate 動態載入 DLL，呼叫 SU API 函數讀取模型資料。

## 修改的檔案清單

### 1. `crates/skp/` — SKP SDK crate（核心）

#### `crates/skp/Cargo.toml`
- 定義 `kolibri-skp` crate
- 依賴：`libloading 0.8`（DLL 動態載入）、`kolibri-core`、`serde`、`thiserror`

#### `crates/skp/src/lib.rs` — 公開 API
- `sdk_available() -> bool`：檢查 DLL 是否可載入
- `import_skp(path) -> Result<SkpScene, SkpError>`：匯入 SKP 檔
- 資料結構：
  - `SkpScene`：meshes, instances, groups, component_defs, materials
  - `SkpMesh`：id, name, vertices(`Vec<[f32;3]>`), normals, indices(`Vec<u32>`), material_id
  - `SkpInstance`：id, mesh_id, component_def_id, transform(`[f32;16]`), name, layer
  - `SkpGroup`：id, name, children, parent_id
  - `SkpComponentDef`：id, name, mesh_ids, instance_count
  - `SkpMaterial`：id, name, color
- `SkpError`：thiserror 列舉（SdkNotFound, OpenFailed, SdkError）

#### `crates/skp/src/ffi.rs` — SketchUp C API FFI 綁定
- **動態載入**：`libloading::Library::new()` 搜尋多個路徑
- **DLL 搜尋順序**：
  1. `SketchUpAPI.dll`（當前目錄）
  2. `sketchup_sdk/SketchUpAPI.dll`
  3. `./lib/SketchUpAPI.dll`
  4. `C:/Program Files/SketchUp/SketchUp 2025/SketchUp/SketchUpAPI.dll`
  5. 執行檔目錄、工作目錄
- **SU 型別**（opaque pointer handles）：
  - `SUModelRef`, `SUEntitiesRef`, `SUFaceRef`, `SUVertexRef`
  - `SUComponentDefinitionRef`, `SUComponentInstanceRef`, `SUGroupRef`
  - `SUMaterialRef`, `SUMeshHelperRef`, `SUStringRef`
  - `SUPoint3D`, `SUVector3D`, `SUTransformation`, `SUColor`
- **載入的 API 函數**（30+）：
  - Model：`SUInitialize`, `SUTerminate`, `SUModelCreateFromFile`, `SUModelRelease`, `SUModelGetEntities`
  - Entities：`SUEntitiesGetNumFaces/GetFaces`, `GetNumGroups/GetGroups`, `GetNumInstances/GetInstances`
  - Face：`SUFaceGetNumVertices`, `SUFaceGetVertices`, `SUFaceGetNormal`
  - MeshHelper（正確三角化凹多邊形）：
    - `SUMeshHelperCreate(face)` → 建立三角化 helper
    - `SUMeshHelperGetNumTriangles/GetNumVertices`
    - `SUMeshHelperGetVertices` → 頂點座標
    - `SUMeshHelperGetVertexIndices` → 三角索引（**0-based**）
    - `SUMeshHelperGetNormals` → 頂點法線
    - `SUMeshHelperRelease`
  - Vertex：`SUVertexGetPosition`
  - Material：`SUMaterialGetName`, `SUMaterialGetColor`（optional，SU2025 可能缺少）
  - ComponentDef：`GetName`, `GetEntities`
  - ComponentInstance：`GetDefinition`, `GetTransform`, `GetName`
  - Group：`GetEntities`, `GetTransform`, `GetName`
  - String：`SUStringCreate/Release/GetUTF8/GetUTF8Length`
- **UTF-8 中文修正**：`string_to_rust()` 用 null-byte 掃描取代 `actual-1`，避免中文字截斷
- **SUFaceGetMaterial**：optional 載入，SU2025 DLL 不一定有此 symbol，用 dummy function fallback

#### `crates/skp/src/converter.rs` — SDK Model → SkpScene 轉換
- **`convert_model(sdk, model) -> SkpScene`**：入口函數
- **`convert_entities(sdk, entities, parent_group_id, world_transform, scene, state, skip_faces)`**：
  - 遞迴處理 entities 樹
  - `world_transform: [f64; 16]` 是累積的 4x4 column-major 矩陣
  - `skip_faces: bool`：component def 的 faces 已處理時跳過，避免重複
  - 處理順序：Faces → Groups → ComponentInstances
- **`mul_transform(a, b) -> [f64; 16]`**：4x4 矩陣乘法（column-major）
- **`faces_to_mesh(sdk, faces, world_xf, state) -> SkpMesh`**：
  - 用 `SUMeshHelperCreate` 對每個 face 正確三角化（支援凹多邊形）
  - 頂點套用 world transform + inch→mm（×25.4）
  - **座標軸交換**：SU(X,Y,Z) → Kolibri(X,Z,Y)（`vertices.push([wx, wz, wy])`）
  - 法線也做座標軸交換 + transform rotation
  - 索引是 **0-based**（MeshHelper 回傳值直接使用）
- **關鍵修正**：
  - Component faces 不重複：遞迴 component 子 entities 時 `skip_faces=true`
  - MeshHelper 索引 0-based（不做 -1）

### 2. `app/src/app.rs` — APP ImportFile 處理器

#### `McpCommand::ImportFile { path }` handler（~line 2470-2523）
- SKP 匯入流程：
  1. `kolibri_skp::import_skp(&path)` 取得 `SkpScene`
  2. 建立 `mesh_map: HashMap<&str, &SkpMesh>` 查詢表
  3. 遍歷 `skp_scene.instances`（不是 meshes），每個 instance：
     - 套用 `inst.transform` 到頂點（4x4 矩陣乘法）
     - 建立 `HeMesh`：`add_vertex()` + 直接 `faces.insert()` with `vert_ids`
     - 每個三角形用 cross product 計算法線
     - `scene.insert_mesh_raw(name, [0,0,0], he, MaterialKind::White)`
  4. `scene.version += 1` 觸發渲染更新

#### `--auto-mcp` 啟動參數
- 建立 `McpBridge` + 啟動 HTTP bridge server（port 3001）
- 讓外部可透過 HTTP JSON-RPC 控制 APP 場景

### 3. `app/src/mcp_http_bridge.rs` — MCP HTTP 橋接
- `tool_to_command("import_file", args)` → `McpCommand::ImportFile { path }`
- 120 秒 timeout 給大型匯入

### 4. `app/src/renderer.rs` — 渲染器修改

#### 雙面法線（line ~239-241）
```wgsl
let raw_n = normalize(i.normal);
let n = select(-raw_n, raw_n, is_front);
```
- 背面（`!is_front`）時翻轉法線，確保匯入 mesh 兩面都正確受光
- 修正了匯入 mesh 部分面全暗的問題

### 5. `crates/core/src/halfedge.rs` — HeMesh 修改

#### `HeFace.vert_ids: Option<Vec<VId>>`
- 新增欄位：直接存三角形頂點索引，跳過慢的半邊拓撲建立
- `face_vertices()` 優先檢查 `vert_ids`（快速路徑），fallback 到 edge walk
- `next_fid()`：公開方法，直接分配 face ID

#### `all_edge_segments()` 支援 vert_ids
- 半邊拓撲存在時用原方法（edges + twins）
- vert_ids 模式：從每個 face 的頂點對生成邊
- **共面邊過濾**：兩個相鄰面法線夾角 < 2.5° 的邊不顯示（三角化對角線隱藏）

### 6. `app/src/overlay.rs` — 頂點編號除錯
- `show_vertex_ids` 開關：在每個頂點位置顯示 VId 紅色標籤 + 黃色圓點
- 用 `world_to_screen_vp()` 投影 3D → 2D

### 7. `app/src/viewer.rs` — ViewerState
- 新增 `show_vertex_ids: bool` 欄位

### 8. `app/src/panels.rs` — 面板 UI
- DISPLAY 區塊新增「頂點編號」checkbox
- 線粗 slider 下限改為 0.1（原 0.5）

## 資料流

```
SKP 檔案
  ↓ libloading 載入 SketchUpAPI.dll
  ↓ SUModelCreateFromFile
  ↓ SUModelGetEntities
  ↓ convert_entities（遞迴）
  ↓   ├─ get_faces → SUMeshHelperCreate → 頂點+索引+法線
  ↓   ├─ get_groups → 遞迴（累積 transform）
  ↓   └─ get_component_instances → 遞迴（累積 transform, skip_faces=true）
  ↓ SkpScene { meshes, instances, groups, component_defs }
  ↓
APP McpCommand::ImportFile
  ↓ instances 遍歷（每個帶 transform）
  ↓ HeMesh 建立（vert_ids 快速路徑）
  ↓ Scene.insert_mesh_raw()
  ↓
wgpu PBR 渲染
  ↓ face_vertices() 從 vert_ids 取頂點
  ↓ all_edge_segments() 從 vert_ids 生成邊（過濾共面對角線）
  ↓ fs_main() 雙面法線光照
```

## 座標系統

| 軸 | SketchUp | Kolibri |
|---|----------|---------|
| X | 右（紅） | 右（紅） |
| Y | 進（綠） | **上（綠）** |
| Z | 上（藍） | **進（藍）** |

轉換：`SU(X,Y,Z) → Kolibri(X, Z, Y)`，單位 inch→mm（×25.4）

## 已知問題

1. **材質未提取**：`SUFaceGetMaterial` 在 SU2025 不可用，全部用 `MaterialKind::White`
2. **面上多餘對角線**：三角化內部邊（已用共面法線過濾修正）
3. **中間無編碼區域**：某些面片跨越大範圍但中間無頂點，可能是三角化產物或缺失幾何，待查
