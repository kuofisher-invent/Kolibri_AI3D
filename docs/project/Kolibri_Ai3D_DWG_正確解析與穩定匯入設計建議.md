# 《Kolibri Ai3D：DWG 正確解析與穩定匯入設計建議》v1.0

---

## 🎯 目標

建立一套讓 **Kolibri Ai3D 能正確解析 DWG 檔案** 的穩定流程，避免目前出現的問題：

- 匯入後場景是空的
- 物件邊界框異常巨大
- 模型位置遠離原點
- 模型尺寸異常縮小
- 相機看不到真正物件
- 匯入成功但實際上只建立了一個錯誤的 box

---

## 🧠 目前問題的本質

從現況來看，問題不在相機，也不在 viewport。

### 真正的核心問題是：
**DWG 並沒有被正確解析成幾何資料。**

目前的 `smart_import` 看起來較像是：

```text
DWG binary → 掃描數值 → 猜測座標 → 組出 bbox → 建一個 generic mesh / box
```

這種方式會導致兩種極端：

### 問題 1：誤把非座標資料當成座標
結果：
- bbox 超大
- 模型離原點很遠
- 相機怎麼 zoom 都不對

### 問題 2：過濾條件太嚴
結果：
- 真正幾何被濾掉
- 只剩超小物件
- 場景看起來幾乎空白

---

## ❌ 不建議再繼續做的修法

以下做法只能暫時碰運氣，不是根治：

- 一直調整座標上限，例如 `1_000_000 → 50_000`
- 一直手動改相機 distance
- 匯入完直接用 bbox 強行居中
- 只靠 screenshot 判斷匯入結果
- 繼續用 binary scan 去猜 DWG 幾何

---

## ✅ 正確方向總結

### Kolibri 應改成以下策略：

```text
DWG → 正式解析（或先轉 DXF）→ 幾何清洗 → 合理性檢查 → IR → 建模 → 顯示
```

---

## 🚀 建議的最佳實作路線

# 方案 A（最穩定，強烈建議）
## DWG 先轉 DXF，再由 Kolibri 解析 DXF

### 流程
```text
.dw g → converter → .dxf → dxf_importer → Kolibri IR → Scene
```

### 優點
- DXF 為文字格式，容易做 parser
- 可直接讀取 LINE / POLYLINE / INSERT / TEXT / DIMENSION
- 不需要用 binary scan 猜測座標
- 匯入結果穩定很多
- 更容易做 grid / 柱位 / 梁線辨識

### 適合現在的 Kolibri 階段
非常適合，因為你目前正在建立自己的 scene graph 與 semantic builder，
先用 DXF 作為穩定入口，開發效率會高很多。

---

# 方案 B（產品表面支援 DWG）
## UI 上支援 DWG，但內部仍先轉 DXF

### 使用者視角
```text
選擇 DWG → Kolibri 顯示「正在匯入 DWG」
```

### 系統內部實際動作
```text
1. 檢查副檔名 .dwg
2. 呼叫外部轉檔器
3. 生成暫存 .dxf
4. 交給 dxf_importer
5. 刪除或保留暫存檔
```

### 優點
- 使用者仍然覺得產品支援 DWG
- 開發端可先用成熟穩定的 DXF 路線
- 容易 debug 與記錄中介結果

---

## 🧩 匯入流程建議

```text
1. 選擇 DWG
2. 嘗試轉 DXF
3. 解析 DXF entities
4. 清洗幾何資料
5. 執行 bbox / scale 合理性檢查
6. 轉成 Kolibri IR
7. 顯示解析預覽
8. 確認後建立模型
```

---

## 📦 建議新增的模組

```text
src/
├── import/
│   ├── import_manager.rs
│   ├── dwg_importer.rs
│   ├── dxf_importer.rs
│   ├── dxf_converter.rs
│   └── import_validator.rs
│
├── ir/
│   ├── mod.rs
│   ├── geometry_ir.rs
│   ├── scene_ir.rs
│   └── semantic_ir.rs
│
├── parsers/
│   ├── entity_parser.rs
│   ├── grid_parser.rs
│   ├── dimension_parser.rs
│   ├── text_parser.rs
│   └── steel_parser.rs
│
├── cleanup/
│   ├── outlier_filter.rs
│   ├── origin_normalizer.rs
│   └── bbox_analyzer.rs
│
└── builders/
    ├── scene_builder.rs
    └── steel_builder.rs
```

---

## 🧠 Kolibri IR（中介資料）建議

不要在 importer 直接建 3D 物件。

應先建立 IR：

```json
{
  "units": "mm",
  "source_format": "dwg",
  "curves": [],
  "texts": [],
  "dimensions": [],
  "blocks": [],
  "meshes": [],
  "metadata": {}
}
```

若是鋼構圖，可再加 semantic layer：

```json
{
  "grids": [],
  "columns": [],
  "beams": [],
  "plates": [],
  "levels": []
}
```

### IR 的好處
- 匯入與建模分離
- parser 容易 debug
- AI 可以插在 IR 層做補完
- 不會一讀錯就直接污染 scene

---

## 🔍 解析階段要做什麼

# 1. Entity Parser
至少先支援：

- LINE
- LWPOLYLINE
- POLYLINE
- INSERT
- TEXT
- MTEXT
- DIMENSION

### 第一版先不要追求全部 DWG 物件
先把真正常用且可用來建模的部分抓穩。

---

# 2. Grid Parser
辨識：

- 軸線
- 軸號
- 尺寸鏈

輸出：

```json
{
  "grid_x": [],
  "grid_y": []
}
```

---

# 3. Steel Parser
若圖面是鋼構放樣圖：

- 柱位
- 梁線
- 標高
- 柱腳 / base
- 構件方向

---

# 4. Text / Dimension Parser
解析：

- 尺寸數值
- 標高文字（例如 +4200）
- grid 名稱（A、B、C、1、2）
- 圖紙分類文字（柱位圖、立面圖）

---

## 🧹 幾何清洗（非常重要）

這一段是現在最缺的。

### 匯入後不要立刻拿全部座標做 bbox
先清洗資料。

---

# A. 離群值過濾（Outlier Filter）

不要用單一固定閾值，例如：

```rust
if x.abs() > 50000.0 { discard }
```

這種方式太死。

### 建議改成統計法
可採用：

- median / IQR
- clustering（例如 DBSCAN）
- 最大主群集法
- 主要 bbox 群聚法

### 目的
找出真正主幾何群，而不是被單一異常點拉爆。

---

# B. 單位合理化
如果單位未知，至少先做推估：

- 常見 CAD 圖面為 mm
- 若尺寸落在極小尺度，可懷疑是 meter / inch 轉換錯誤
- 匯入後加單位判斷報告

---

# C. 原點正規化（Origin Normalize）
只有在找出主幾何群之後才做：

```text
主群集中心 → 平移到 Kolibri 近原點
```

### 不要在亂資料上直接 normalize
否則只是把錯誤模型搬到另一個錯誤位置。

---

## 📏 合理性檢查（Import Validator）

這一層一定要做。

匯入完先檢查：

- bbox_x
- bbox_y
- bbox_z
- object_count
- curve_count
- dimension_count
- 是否存在主群集

### 範例規則
```text
若 bbox_x < 1000 mm 或 > 100000 mm → suspicious
若 bbox_z < 1000 mm 或 > 100000 mm → suspicious
若 object_count = 1 且 shape_type = box 且 curves 遠多於 meshes → suspicious
```

### suspicious 時不要直接建模
而是顯示解析預覽與警告。

---

## 🖥️ UI 建議：匯入解析預覽面板

匯入後先顯示：

```text
匯入來源：采鈺-龍潭裝修-放樣圖-0305.dwg
格式：DWG（內部轉 DXF）
偵測到：
- Curves: 153
- Texts: 24
- Dimensions: 8
- 主幾何範圍：X=9880, Y=4200, Z=2950

狀態：正常 / 可疑

[建立模型] [重新解析] [輸出 IR Debug]
```

如果發現異常：

```text
警告：匯入尺度異常
- bbox_x = 287512 mm
- bbox_z = 548870 mm

可能原因：
1. DWG 解析錯誤
2. 離群值污染
3. 單位判定錯誤
```

---

## 🏗️ 建模器設計建議

匯入器不應該直接建一個 generic box。

### 正確做法：
Importer → IR → Builder

---

# Scene Builder
將：
- curves
- blocks
- meshes

轉成 Kolibri 的顯示物件。

---

# Steel Builder
若圖面判定為鋼構：

- grid → 建立軸線
- 柱位 → 建立柱
- 梁線 → 建立梁
- 標高 → 設定構件高度

### 這比 generic mesh 更有價值
也更接近你未來 Tekla-like 路線。

---

## 🤖 AI 在 DWG 解析中的正確位置

AI 不該直接讀 binary 或直接建模。

### AI 最適合做：
1. 圖紙分類
2. 語意補全
3. 缺失資訊推測
4. 解析錯誤修正建議

### 例如
- 這張圖是柱位圖還是立面圖
- 哪些文字是 grid label
- 哪些尺寸是標高
- 哪些線段可能是構件中心線

---

## ⚠️ 目前根因與修復建議對照表

| 問題 | 根因 | 建議修法 |
|------|------|----------|
| 匯入後看不到物件 | bbox 錯誤 / 位置錯誤 | 正式 parser + 合理性檢查 |
| 模型超大 | 非座標資料誤判 | 停用 binary scan，改走 DXF |
| 模型超小 | 過濾太嚴 | 改用統計式離群值過濾 |
| 匯入成功但只有 1 個 box | 沒真正解析成幾何/語意 | 建立 IR + Builder |
| 相機看不到全貌 | 物件本身錯誤 | 修 importer，不是先調 camera |

---

## ✅ 建議的開發順序

# Phase 1：止血
1. 停止用 binary scan 猜 DWG 幾何
2. 建立 DWG → DXF 路線
3. 加入 bbox 合理性檢查
4. 顯示匯入預覽，不直接建模

---

# Phase 2：穩定匯入
1. 完成 DXF entity parser
2. 建立 IR
3. 做 outlier filter
4. 做 origin normalize

---

# Phase 3：智能解析
1. grid parser
2. dimension parser
3. steel parser
4. AI 補全

---

# Phase 4：工程建模
1. scene builder
2. steel builder
3. profile / material mapping
4. 後續 BOM / connection / drawing export

---

## 📌 MVP 建議

如果你現在就要做一個能工作的版本，建議先做到：

### MVP v1
- DWG 進來先轉 DXF
- 解析 LINE / TEXT / DIMENSION
- 建立 IR
- 檢查 bbox 是否合理
- 顯示解析預覽
- 再建立簡單線框 / member 模型

### 先不要做
- 直接啃 DWG binary
- 一匯入就自動建 box
- 一開始就追求完整 DWG 所有物件支援

---

## 🧠 關鍵結論

Kolibri 如果要「正確解析 DWG」，真正要修的不是 camera，不是 zoom，不是 viewport。

### 真正要修的是：
- 匯入策略
- parser 路線
- 幾何清洗
- 合理性檢查
- IR 與 Builder 分層

---

## 📌 一句話總結

**DWG 正確解析的關鍵，不是把錯誤座標修得比較像，而是停止猜測，改走可驗證、可清洗、可建模的正式流程。**

---
