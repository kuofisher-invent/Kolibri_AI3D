
# K3D 鋼構加工外掛設計草案  
**文件版本：v0.1**  
**用途：作為 K3D 的 Steel Fabrication Plugin 開發藍圖**

---

## 1. 目標定位

本外掛的目的，不是把鋼構加工邏輯寫死在 K3D 核心內，而是以 **外掛（Plugin）** 方式擴充 K3D，讓 K3D 維持：

- 幾何建模核心
- inference / semantic 判斷核心
- scene graph / UI 核心

而鋼構加工相關能力，則由外掛負責提供：

- 構件辨識
- 接頭生成
- 板件展開
- DXF / 加工圖輸出
- 未來接機器人焊接路徑

---

## 2. 核心設計理念

### 2.1 K3D 與外掛分工

```text
[K3D Core]
  ├─ Geometry Engine
  ├─ Inference Engine
  ├─ Scene Graph
  ├─ Selection / Viewport / UI
  └─ Plugin API

        ↓

[Steel Fabrication Plugin]
  ├─ 構件辨識
  ├─ 接頭規則引擎
  ├─ Plate 展開
  ├─ 加工圖輸出
  └─ Robot Path（未來）
```

### 2.2 設計原則

1. **K3D 核心保持通用**
   - 不把鋼構專用邏輯硬寫進主程式。
2. **鋼構邏輯全部外掛化**
   - 接頭、孔位、板厚、展開規則皆由 plugin 管理。
3. **先做半自動，不追求全自動**
   - 系統先判斷，使用者確認。
4. **先做平板與基礎接頭**
   - 第一版不碰曲面展開、不碰太複雜節點。
5. **先做到可加工圖輸出**
   - 先解決「無法代加工」問題，再往機器人焊接延伸。

---

## 3. 外掛目標問題

K3D Steel Plugin 主要要補以下缺口：

### 3.1 建築圖 ≠ 鋼構構件模型
建築師提供的圖面常缺乏以下資訊：

- 梁 / 柱 / 板之語意
- 接頭型式
- 孔位與板厚
- 焊接資訊
- 加工可行性

### 3.2 3D 模型 ≠ 加工圖
即使已有 3D 模型，也不代表能直接加工，仍缺：

- 板件展開圖
- 開孔圖
- 零件編號
- 加工圖輸出格式（DXF 等）

### 3.3 加工圖 ≠ 機器手臂動作
若未來接 UR 焊接機器人，還需要：

- 焊道識別
- 焊接路徑生成
- torch 姿態
- URScript / 路徑輸出

---

## 4. 外掛功能範圍

### 4.1 第一階段（MVP）
建議先做三項：

1. **柱 / 梁 / 板 的構件辨識**
2. **Base Plate 或簡單端板接頭自動生成**
3. **DXF 加工圖輸出**

### 4.2 第二階段
1. **Beam-Column 接頭規則擴充**
2. **孔位 / 螺栓 / 焊接資訊**
3. **零件命名與編號**

### 4.3 第三階段
1. **加工流程規劃**
2. **焊接路徑生成**
3. **UR 機器手臂路徑輸出**

---

## 5. 外掛專案目錄建議

```text
k3d-steel-plugin/
├── plugin.json
├── main.py
├── modules/
│   ├── recognizer.py
│   ├── joint_engine.py
│   ├── unfolding.py
│   ├── exporter.py
│   ├── naming.py
│   └── robot_path.py
├── rules/
│   ├── joints/
│   │   ├── base_plate.json
│   │   ├── end_plate.json
│   │   └── splice_plate.json
│   └── profiles/
│       ├── h_beam.json
│       ├── box_column.json
│       └── plate.json
└── docs/
    └── DEVELOPMENT_NOTES.md
```

---

## 6. plugin.json 建議格式

```json
{
  "name": "k3d-steel-fabrication",
  "version": "0.1.0",
  "entry": "main.py",
  "permissions": [
    "scene.read",
    "scene.write",
    "geometry.analyze",
    "selection.read",
    "ui.panel",
    "file.export"
  ]
}
```

### 欄位說明

- `name`：外掛名稱
- `version`：版本
- `entry`：主入口檔案
- `permissions`：外掛允許使用的 K3D API 權限

---

## 7. K3D 核心需提供的 Plugin API

Steel Plugin 能否成功，關鍵不在 plugin 本身，而在 **K3D 是否有足夠的 plugin API**。

---

### 7.1 Scene API

```python
scene.get_selected_objects()
scene.get_all_objects()
scene.get_object_by_id(object_id)
scene.create_object(object_type, params)
scene.update_object(object_id, data)
scene.delete_object(object_id)
```

用途：
- 讀取目前場景
- 建立 plate / bolt / hole 等物件
- 更新構件資訊

---

### 7.2 Geometry API

```python
geom.get_edges(obj)
geom.get_faces(obj)
geom.get_vertices(obj)
geom.get_bbox(obj)
geom.get_center(obj)
geom.get_intersections(obj_a, obj_b)
geom.get_face_normal(face)
geom.project_to_plane(points, plane)
```

用途：
- 取得構件尺寸
- 判斷交接位置
- 做 plate 投影與展開

---

### 7.3 Inference / Semantic API

```python
inference.get_semantic_type(obj)
inference.get_context()
inference.get_score_breakdown(obj)
```

用途：
- 判斷 beam / column / plate
- 利用既有 inference 引擎做語意加權

---

### 7.4 Selection API

```python
selection.get_current()
selection.set_current(object_ids)
```

用途：
- 使用者選柱 + 梁後，交由外掛處理

---

### 7.5 UI API

```python
ui.add_panel(panel_id, title)
ui.add_button(panel_id, label, callback)
ui.add_dropdown(panel_id, label, options, callback)
ui.show_message(text)
ui.show_confirm(text)
ui.show_table(data)
```

用途：
- 在 K3D 內增加一個 Steel 分頁或 Steel 面板
- 提供簡單交互

---

### 7.6 Export API

```python
export.save_text(filename, content)
export.save_dxf(filename, entities)
export.save_json(filename, data)
```

用途：
- 輸出加工圖
- 匯出中間結果供後端或機器人使用

---

## 8. 外掛模組設計

---

### 8.1 recognizer.py：構件辨識模組

用途：
- 將一般幾何物件辨識為柱、梁、板等構件。

#### 初期建議：rule-based
第一版先用規則，不用急著上 AI。

#### 範例流程
1. 取 bbox
2. 比較長寬高
3. 比對方向
4. 加上 scene / inference 上下文

#### 範例程式概念

```python
def detect_member(obj):
    bbox = geom.get_bbox(obj)

    if bbox.height > bbox.width and bbox.height > bbox.depth:
        return "column"
    elif bbox.length > bbox.height:
        return "beam"
    else:
        return "plate"
```

#### 後續可升級
- profile 比對（H 型鋼、箱型柱）
- 與使用者命名規則結合
- 透過 inference engine 加權判斷

---

### 8.2 joint_engine.py：接頭引擎

用途：
- 判斷兩個構件的連接形式
- 自動生成 plate / 孔位 / 螺栓配置

#### MVP 建議支援
1. Base Plate
2. Beam-Column End Plate
3. 柱拼接 splice（後期）

#### 範例流程
1. 取得 beam 與 column
2. 找交點
3. 判斷關係（垂直 / 共線 / 側接）
4. 套用規則
5. 產生 plate 幾何與 metadata

#### 範例程式概念

```python
def create_beam_column_joint(beam, column):
    intersection = geom.get_intersections(beam, column)

    return {
        "type": "end_plate",
        "plate_thickness": 12,
        "bolt_pattern": "4-M20",
        "weld_type": "fillet",
        "intersection": intersection
    }
```

---

### 8.3 unfolding.py：板件展開模組

用途：
- 將 plate 轉成 2D 可輸出外框

#### MVP 範圍
- 只支援 **平板**
- 不支援自由曲面
- 不支援複雜折板

#### 輸出內容
- 外框輪廓
- 孔位
- 厚度
- 零件代號

#### 範例程式概念

```python
def unfold_plate(plate):
    faces = geom.get_faces(plate)
    projected_outline = geom.project_to_plane(faces, plane="XY")

    return {
        "outline": projected_outline,
        "holes": [],
        "thickness": 12
    }
```

---

### 8.4 exporter.py：加工圖輸出

用途：
- 輸出給雷切 / CNC / 加工廠使用

#### MVP 建議
- DXF
- JSON（中間資料）

#### 範例程式概念

```python
def export_dxf(data, filename):
    # 將 outline、holes 轉成 DXF entity
    pass
```

#### 建議輸出內容
- outer contour
- hole circles
- part name
- thickness
- material

---

### 8.5 naming.py：命名與編號模組

用途：
- 把 `comp_xxxxx` 轉為可理解名稱

#### 建議規則
- 柱：C-01, C-02
- 梁：B-01, B-02
- 板：P-01, P-02
- Base Plate：BP-01

#### 範例

```python
def generate_part_name(part_type, index):
    prefix_map = {
        "column": "C",
        "beam": "B",
        "plate": "P",
        "base_plate": "BP"
    }
    return f"{prefix_map[part_type]}-{index:02d}"
```

---

### 8.6 robot_path.py：機器人路徑模組（後期）

用途：
- 將接頭焊道轉為機器手臂可用路徑

#### 第一版不做
這模組先保留介面，不急著完成。

#### 未來輸入
- 接頭幾何
- 焊道位置
- 焊接順序
- 工件座標

#### 未來輸出
- path points
- torch orientation
- URScript

---

## 9. 規則資料設計（Rule Data）

接頭規則不要寫死在程式裡，建議做成 JSON / YAML。

---

### 9.1 base_plate.json 範例

```json
{
  "joint_type": "base_plate",
  "plate_thickness": 16,
  "bolt_count": 4,
  "bolt_diameter": 20,
  "edge_distance": 40,
  "default_weld": "fillet"
}
```

---

### 9.2 end_plate.json 範例

```json
{
  "joint_type": "end_plate",
  "plate_thickness": 12,
  "bolt_pattern": "2x2",
  "bolt_diameter": 20,
  "default_weld": "fillet"
}
```

---

## 10. 使用流程（Workflow）

---

### 10.1 Base Plate 生成流程

```text
1. 使用者選取柱
2. 點擊「Steel Plugin → 生成 Base Plate」
3. 系統讀取柱截面尺寸
4. 套用 base_plate 規則
5. 生成 plate 幾何
6. 使用者確認
7. 輸出 DXF
```

---

### 10.2 Beam-Column 接頭流程

```text
1. 使用者選取梁 + 柱
2. 點擊「Steel Plugin → 生成接頭」
3. 系統判斷交接方式
4. 推薦 end plate / shear tab / 其他規則
5. 使用者確認
6. 生成構件
7. 匯出加工圖
```

---

## 11. UI 整合建議

### 11.1 不新增複雜 UI 系統
建議直接整合到 K3D 現有右側面板或模式面板中。

### 11.2 建議新增一個 Steel 分頁
與：
- 建模
- BIM
- 法規檢核

並列成第四個分頁：

- **Steel**

---

### 11.3 Steel 分頁最小介面

#### 區塊一：辨識
- 辨識構件
- 自動命名

#### 區塊二：接頭
- 生成 Base Plate
- 生成 Beam-Column 接頭

#### 區塊三：輸出
- 輸出 DXF
- 匯出 JSON

#### 區塊四：未來
- 產生焊接路徑
- 匯出 URScript

---

## 12. MVP 開發順序建議

建議依照以下順序開發，不要一次做太多：

### Phase 1：構件辨識
- beam / column / plate
- 命名規則

### Phase 2：Base Plate
- 自動生成
- 2D 展開
- DXF 輸出

### Phase 3：Beam-Column 接頭
- 簡單端板規則
- 孔位與螺栓

### Phase 4：BIM / Metadata
- 將鋼構構件同步到 BIM / IFC 類型

### Phase 5：Robot Path
- 焊道抽取
- 路徑輸出

---

## 13. 風險與限制

### 13.1 不要一開始就追求全自動
鋼構接頭有太多現場差異，初期應做成：

- 系統判斷
- 使用者確認
- 系統生成

也就是 **半自動決策模式**。

### 13.2 不要一開始就做複雜曲面展開
第一版只碰：
- 平板
- Base Plate
- 簡單端板

### 13.3 不要太早綁死國家規範
規則引擎要可換：
- 台灣
- AISC
- 自訂工廠規則

---

## 14. 未來擴充方向

### 14.1 規範化
- 台灣鋼構常用接頭規則庫
- AISC 規則套件
- 工廠自訂模板

### 14.2 BIM 化
- IfcBeam
- IfcColumn
- IfcPlate
- 關聯 metadata

### 14.3 製造自動化
- 工件排序
- 焊接順序
- 路徑規劃
- 與 UR 手臂整合

---

## 15. 建議結論

K3D 不應只是一套建模工具。  
若要真正補足你目前在鋼構工作上的痛點，建議方向是：

> **K3D Core 保持通用建模平台**
>  
> **Steel Plugin 負責把建築幾何轉成鋼構可加工資訊**

也就是：

```text
建築圖 / 幾何
    ↓
K3D
    ↓
Steel Plugin
    ↓
構件辨識 / 接頭生成 / 展開
    ↓
DXF / 加工圖 / 未來機器人焊接
```

這樣的架構有幾個好處：

1. 不會把 K3D 核心污染成鋼構專用軟體
2. 鋼構邏輯可以獨立演進
3. 未來可擴充其他 plugin（木構、鋁門窗、消防檢核等）
4. 最後能形成真正的產業平台，而不是單一工具

---

## 16. 下一步建議

若要正式進入開發，建議下一份文件可直接接著寫：

1. **Steel Plugin API Spec v1**
2. **Base Plate 規則庫設計**
3. **DXF Export 資料格式規格**
4. **Beam-Column Joint Engine 詳細設計**
5. **UR 焊接路徑輸出資料模型**

---
