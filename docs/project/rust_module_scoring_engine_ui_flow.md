# 《Kolibri Inference 2.0 架構設計（Rust module + scoring engine + UI flow）》v1.0

---

## 🎯 目標

為 Kolibri Ai3D 建立一套 **Inference 2.0** 架構，讓系統不只做傳統幾何吸附（endpoint / midpoint / axis），而是進一步具備：

- 幾何推論（Geometry Inference）
- 上下文推論（Context-aware Inference）
- 語意建議（Semantic Suggestion）
- 使用者確認（Human-in-the-loop）
- 工程建模輔助（Beam / Column / Plate / Grid）

最終目標：

👉 **從「找到最近的點」進化成「猜到使用者要什麼」**

---

## 🧠 核心理念

### Inference 1.0（傳統）

- endpoint
- midpoint
- intersection
- axis lock
- face snap

本質：

👉 幾何規則（rule-based）

---

### Inference 2.0（Kolibri）

- 幾何候選
- 上下文加權
- 語意打分
- AI / 規則建議
- 使用者確認

本質：

👉 **幾何 + 上下文 + 語意 + 使用者意圖**

---

## 🧩 系統總體架構

```text
Input Event
  ↓
Geometry Candidate Collector
  ↓
Context Analyzer
  ↓
Scoring Engine
  ↓
Inference Result
  ↓
UI Presenter
  ↓
User Confirm / Ignore / Override
```

若為匯入模型語意分析：

```text
Imported Geometry IR
  ↓
Feature Extractor
  ↓
Semantic Scoring Engine
  ↓
Semantic Candidates
  ↓
Semantic Review UI
  ↓
Confirmed Semantic Model
```

---

# 1. Rust 模組架構

```text
src/
├── inference/
│   ├── mod.rs
│   ├── types.rs
│   ├── geometry_candidates.rs
│   ├── context.rs
│   ├── scoring.rs
│   ├── semantic_scoring.rs
│   ├── suggestions.rs
│   └── resolver.rs
│
├── ui/
│   ├── cursor_hint.rs
│   ├── semantic_review_panel.rs
│   └── inference_overlay.rs
│
├── features/
│   ├── geometry_features.rs
│   ├── topology_features.rs
│   └── semantic_features.rs
```

---

# 2. 核心資料結構

## 2.1 基本推論種類

```rust
pub enum InferenceType {
    Endpoint,
    Midpoint,
    Intersection,
    AxisLockX,
    AxisLockY,
    AxisLockZ,
    OnFace,
    Parallel,
    Perpendicular,
    GridLine,
    BeamAxis,
    ColumnAxis,
    SemanticSuggestion,
}
```

---

## 2.2 幾何候選

```rust
pub struct InferenceCandidate {
    pub id: String,
    pub inference_type: InferenceType,
    pub position: [f32; 3],
    pub score: f32,
    pub reasons: Vec<String>,
}
```

---

## 2.3 使用者互動上下文

```rust
pub struct InteractionContext {
    pub current_tool: ToolKind,
    pub selected_object_ids: Vec<String>,
    pub hover_object_id: Option<String>,
    pub last_direction: Option<[f32; 3]>,
    pub working_plane: WorkingPlane,
    pub input_mode: InputMode,
}
```

---

## 2.4 語意建議

```rust
pub struct SemanticSuggestion {
    pub object_id: String,
    pub suggested_kind: SemanticKind,
    pub confidence: f32,
    pub reasons: Vec<String>,
}
```

---

## 2.5 語意狀態

```rust
pub enum SemanticState {
    None,
    Suggested,
    Confirmed,
    Rejected,
    Modified,
}
```

---

# 3. 幾何候選收集（Geometry Candidate Collector）

## 功能

負責從場景中找出所有可能可吸附 / 可推論的候選：

- endpoint
- midpoint
- edge direction
- face normal
- grid line
- object axis

---

## 流程

```text
mouse position
  ↓
ray cast
  ↓
nearby edges / faces / points
  ↓
generate candidates
```

---

## 輸出範例

```json
[
  {"type": "Endpoint", "position": [0,0,0]},
  {"type": "Midpoint", "position": [500,0,0]},
  {"type": "OnFace", "position": [250,0,250]}
]
```

---

# 4. Context Analyzer（上下文分析）

## 功能

決定「目前哪種推論比較重要」。

例如：

- 正在畫線 → endpoint / axis 優先
- 正在放梁 → beam axis / grid line 優先
- 正在匯入語意確認 → semantic suggestion 優先

---

## 可分析項目

- current_tool
- working_plane
- last_direction
- selected object kind
- current mode（建模 / 鋼構）
- import review state

---

## 範例規則

### 畫線中
- endpoint +30
- axis continuation +40
- on same plane +25

### 放梁中
- grid line +40
- horizontal axis +30
- connects two column candidates +30

---

# 5. Scoring Engine（打分引擎）

## 核心概念

不要用「最近點」決定推論，改用：

```text
總分 = 幾何分 + 上下文分 + 語意分 + 意圖分
```

---

## 建議公式

```text
score = geometry_score + context_score + semantic_score + intent_score
```

---

## 5.1 Geometry Score

依幾何事實計分：

- 距離近
- 在當前平面上
- 角度接近
- 是否為明確特徵點

---

## 5.2 Context Score

依當前工具與操作脈絡計分：

- 当前 tool 是否偏好這類候選
- 是否延續上一步方向
- 是否靠近已選取物件

---

## 5.3 Semantic Score

依工程語意計分：

- 是否可能是 beam axis
- 是否是 column center
- 是否對齊 grid

---

## 5.4 Intent Score

根據使用者近期操作推測意圖：

- 連續畫牆
- 連續放柱
- 正在修正特定區域

---

## Rust 介面建議

```rust
pub trait ScoreRule {
    fn score(&self, candidate: &InferenceCandidate, ctx: &InteractionContext) -> f32;
}
```

---

# 6. Semantic Scoring Engine（語意打分）

## 目標

將匯入後的 generic geometry 轉成語意候選：

- Beam
- Column
- Plate
- Grid
- Connection Candidate

---

## 輸入特徵

### Geometry Features
- bounding box
- aspect ratio
- principal axis
- thickness / width / length

### Topology Features
- 是否接地
- 是否連接其他構件
- 是否在同一高度層

### Pattern Features
- 是否等距重複
- 是否平行群組
- 是否落在 grid 交點

---

## Beam 判定範例

| 條件 | 分數 |
|------|------|
| 水平 | +30 |
| 長寬比高 | +20 |
| 連接兩節點 | +20 |
| 與其他梁同標高 | +15 |
| 有 profile-like 斷面 | +15 |

---

## Column 判定範例

| 條件 | 分數 |
|------|------|
| 垂直 | +30 |
| 接地 | +20 |
| 高度明確 | +15 |
| 位於規律交點 | +20 |
| 斷面一致 | +15 |

---

## Plate 判定範例

| 條件 | 分數 |
|------|------|
| 薄片 | +30 |
| 外框封閉 | +25 |
| 厚度遠小於長寬 | +25 |
| 法向量穩定 | +20 |

---

# 7. Resolver（最終決策器）

## 功能

將候選依分數排序後，輸出最終建議：

- Top 1 給游標提示
- Top N 給 Semantic Review UI

---

## 規則

### 游標推論
- 只取最高分候選
- 若前兩名分數太接近，可顯示次選提示

### 語意建議
- 只輸出 confidence > 門檻者
- 低分者進入人工確認列表

---

## 門檻建議

- `>= 80`：高信心
- `60 ~ 79`：中信心
- `< 60`：低信心（只列入待確認）

---

# 8. UI Flow 設計

---

## 8.1 游標提示 UI（Cursor Hint UI）

### 顯示內容

```text
🟢 On Green Axis
↔ 2500 mm
🤖 延續方向
```

---

### 元件
- 第一行：推論類型
- 第二行：即時距離 / 數值
- 第三行：AI / 上下文建議

---

## 8.2 Ghost Line / Preview

當系統判斷出高信心方向時：

- 顯示 ghost line
- 顯示預測輪廓
- 顯示 plane highlight

---

## 8.3 Semantic Review UI

專門處理匯入後未確認的語意。

### 左側清單

```text
未確認語意
- Beam Candidates (6)
- Column Candidates (10)
- Grid Candidates (8)
```

---

### 中央畫面

- 高亮目前候選物件
- 顯示 bounding / 軸線 / 方向箭頭

---

### 右側確認面板

```text
類型： Beam
信心：82
理由：
- 水平構件
- 長寬比高
- 連接兩節點

[ 確認 ] [ 改成 Column ] [ 忽略 ]
```

---

## 8.4 批次確認 Flow

```text
已選 6 個候選
→ 套用 Beam
→ 指定 Profile: H300x150x6x9
→ 指定材質：SS400
```

---

# 9. Human-in-the-loop 設計

## 原則

AI 永遠只做：
- Suggest
- Rank
- Highlight
- Explain

人永遠掌握：
- Confirm
- Reject
- Override

---

## 為什麼重要

- 降低誤判風險
- 提高工程可靠性
- 增加使用者信任
- 降低 parser / model 的完美要求

---

# 10. Console / Debug 輸出設計

Inference 2.0 一定要可觀察。

### 建議輸出

```text
[Inference]
Candidates: 12
Top: On Green Axis (score 88)
Reasons:
- aligned with last direction
- same working plane
- grid continuation
```

### 語意輸出

```text
[Semantic Suggestion]
Beam_03 score=82
- horizontal member
- connects two column candidates
- repeated profile cluster
```

---

# 11. 開發階段建議

## Phase 1：Rule-based Inference 2.0

- geometry candidate collector
- context analyzer
- scoring engine
- cursor hint UI

目標：
先超越傳統最近點吸附。

---

## Phase 2：Semantic Suggestion

- geometry features
- topology features
- beam / column / plate scoring
- semantic review UI

目標：
讓匯入後的 geometry 可被人快速轉成工程模型。

---

## Phase 3：Pattern / Cluster Intelligence

- repeated object detection
- grid clustering
- level grouping
- pattern consistency scoring

目標：
提升批次推論能力。

---

## Phase 4：AI/ML 擴充（可選）

- rule + ML hybrid
- LLM explanation layer
- vision-assisted drawing understanding

目標：
在既有穩定架構上加智慧，不取代基礎系統。

---

# 12. MVP 建議

若你現在要做第一版，建議先完成：

### 必做
- `InferenceCandidate`
- `InteractionContext`
- `ScoreRule`
- cursor hint UI
- top candidate resolver

### 第二波
- `SemanticSuggestion`
- semantic review panel
- batch confirm flow

### 先不要
- 直接上 ML
- 直接讓 AI 自動畫出構件
- 黑箱式語意決策

---

# 13. 預期成效

Inference 2.0 完成後，Kolibri 會從：

### 原本
- 幾何工具
- 只有吸附與畫圖

### 升級為
- 會理解上下文的建模工具
- 會給出語意候選的工程工具
- 會解釋自己為什麼這樣推論的 AI 輔助平台

---

## 📌 一句話總結

👉 **Kolibri Inference 2.0 的核心，不是更會吸附，而是更會理解。**

👉 它要做到的是：

**幾何可推論、語意可建議、使用者可確認、系統可補完。**

---

