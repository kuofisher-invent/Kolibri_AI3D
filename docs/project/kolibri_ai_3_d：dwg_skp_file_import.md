# 《Kolibri Ai3D：DWG / SKP 雙格式匯入架構設計》v1.0

---

## 🎯 目標

建立一套可同時支援 **DWG（CAD）** 與 **SKP（SketchUp）** 的匯入系統，並轉換為 Kolibri Ai3D 的統一資料模型，以支援：

- 3D 視覺建模
- AI 推論（Inference 2.0）
- 鋼構語意建模（Tekla-like）
- 後續加工 / 分析 / 輸出

---

## 🧠 核心設計理念

### ❌ 錯誤做法
- 用單一 parser 同時解析 DWG + SKP
- 直接將原始資料映射到場景

### ✅ 正確做法

👉 **雙匯入器（Importer） + 統一中介模型（IR）**

```
DWG ─┐
     ├── Importer ──► Kolibri IR ──► Builder ──► Scene
SKP ─┘
```

---

## 🧩 系統架構

```
src/
├── import/
│   ├── dwg_importer.rs
│   ├── skp_importer.rs
│   └── import_manager.rs
│
├── ir/
│   ├── mod.rs
│   ├── geometry.rs
│   ├── material.rs
│   ├── scene_graph.rs
│   └── semantic.rs
│
├── builders/
│   ├── scene_builder.rs
│   ├── steel_builder.rs
│   └── cleanup.rs
│
├── parsers/
│   ├── grid_parser.rs
│   ├── dimension_parser.rs
│   └── semantic_parser.rs
```

---

## 📦 DWG 與 SKP 差異（必須理解）

| 項目 | DWG | SKP |
|------|-----|-----|
| 本質 | CAD / 工程圖 | 3D 模型 |
| 幾何 | line / polyline | face / edge |
| 結構 | layer / block | group / component |
| 語意 | 幾乎沒有 | 部分存在 |
| 用途 | 放樣 / 圖面 | 模型製作 |

---

## 🔧 DWG Importer 設計

### 功能

- 讀取：LINE / POLYLINE / TEXT / DIMENSION / INSERT
- 解析圖層（layer）
- 建立 block instance

### 輸出（IR）

```json
{
  "curves": [],
  "texts": [],
  "dimensions": [],
  "blocks": []
}
```

### 進階（V2）

- grid 偵測
- 柱位辨識
- 梁線辨識
- 標高解析

---

## 🧱 SKP Importer 設計

### 功能

- 讀取：faces / edges
- group / component instance
- transform hierarchy
- material

### 輸出（IR）

```json
{
  "meshes": [],
  "instances": [],
  "materials": []
}
```

### 進階（V2）

- component → semantic object
- instance reuse
- layer/tag mapping

---

## 🧠 Kolibri IR（核心）

### Geometry Layer

```json
{
  "nodes": [],
  "meshes": [],
  "curves": []
}
```

### Scene Graph

```json
{
  "instances": [
    {
      "mesh_id": "",
      "transform": []
    }
  ]
}
```

### Material

```json
{
  "materials": []
}
```

---

## 🏗️ Semantic Layer（關鍵升級）

```json
{
  "members": [
    {"type": "beam"},
    {"type": "column"}
  ],
  "plates": [],
  "connections": []
}
```

---

## 🧠 Builder 層

### Scene Builder

將 IR → 可視化場景

### Steel Builder（重要）

將 IR → 結構語意

例如：

```text
線 → 梁
封閉區域 → 板
交點 → 柱
```

---

## 🔄 匯入流程

```
1. 選擇檔案（DWG / SKP）
2. 呼叫對應 importer
3. 轉換為 IR
4. 執行 parser（grid / semantic）
5. 顯示解析預覽
6. 使用者確認
7. Builder 建立模型
```

---

## 🧠 AI 在這裡的角色

AI 不做解析本身，而做：

- 圖紙分類（柱位圖 / 立面圖）
- 語意補全
- 推測缺失資料
- 修正錯誤解析

---

## ⚠️ 風險與對策

| 問題 | 解法 |
|------|------|
| DWG 很髒 | 加 IR 清洗層 |
| SKP 太重 | instance 化 |
| 單位混亂 | 強制 mm normalization |
| 座標錯誤 | world origin reset |

---

## 🚀 開發階段建議

### V1（核心）
- DWG：基本幾何
- SKP：mesh + instance
- IR 建立

### V2（智能）
- grid / dimension / semantic
- AI 輔助

### V3（工程級）
- steel object
- connection
- fabrication data

---

## 📌 一句話總結

👉 **Kolibri = 多格式匯入 + 統一語意模型 + AI 建模引擎**

---

