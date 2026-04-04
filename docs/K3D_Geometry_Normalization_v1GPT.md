# K3D Geometry Normalization v1

## 🎯 目標
將 DXF / Mesh 幾何資料轉換為：
- 最小化資料量
- 拓撲乾淨
- 可語意辨識
- 可參數化重建

---

## 🥇 STEP 1：頂點量化（Vertex Quantization）

### 目的
- 合併接近點
- 降低精度噪音

### 方法
(x, y, z) → (round(x/ε)*ε)

### 建議 ε
- 建築/鋼構：0.001 mm ~ 0.01 mm

---

## 🥈 STEP 2：頂點去重（Deduplication）

### 方法
使用 HashMap 將 Vec3 映射到 index

### 效果
- 減少記憶體
- 建立 index mesh

---

## 🥉 STEP 3：拓撲正規化（Topology Cleanup）

### 包含
- 合併共線邊
- 移除零長邊
- 統一面法向

---

## 🏅 STEP 4：Primitive Detection（語意壓縮核心）

### 偵測項目
- Beam（線性拉伸）
- Plate（共面閉合輪廓）
- Hole（圓/長孔）
- Bolt Pattern（重複圓）

### 核心概念
將多個 face/edge → 單一語意物件

---

## 🏆 STEP 5：參數化重建（Parametric Encoding）

### 範例

```rust
Beam {
  axis: Line,
  profile: H300x150,
  length: 6000,
}
```

---

## 🧠 壓縮效果

| 類型 | 原始 | K3D |
|------|------|-----|
| 檔案大小 | 1MB~10MB | 50KB~500KB |

---

## ⚠️ 安全機制

### 1. Fallback Mesh
```rust
enum Geometry {
  Parametric,
  Mesh
}
```

### 2. Confidence Score
```rust
confidence: 0.82
```

---

## 🧭 最終資料結構

```rust
struct Model {
  primitives: Vec<Primitive>,
}

enum Primitive {
  Beam,
  Plate,
  Hole,
  Mesh,
}
```

---

## 🚀 核心理念

壓縮 ≠ 減少資料  
壓縮 = 提取語意
