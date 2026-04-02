# K3D 鋼構細部設計路線圖 (Steel Detailing Roadmap)

## 目標
在 Kolibri Ai3D 的 3D 鋼構模式中，實現 Tekla Structures 入門級的鋼構細部設計功能。
使用者能在 3D 中建模 → 加接頭 → 自動編號 → 出施工圖 → 匯出 NC/IFC。

---

## 現有基礎 (updated 2026-04-02)

### 已完成
- [x] H 型鋼建模（CNS 386，15 種 H100~H900）
- [x] 方管建模（16 種 □50~□400）
- [x] 圓管建模（15 種 Ø42.7~Ø406.4）
- [x] 柱/梁/斜撐放置（3 組件 H 截面）
- [x] 鋼板放置（矩形 + PushPull 調厚度）
- [x] 軸線系統
- [x] BOM 料表 CSV 匯出（含螺栓表/焊接表/組裝件）
- [x] 材料等級選擇（SN400B/SN490B/SS400/A572）
- [x] 截面參數面板（H/B/tw/tf/kg/m）
- [x] ComponentKind 標籤（Column/Beam/Brace/Plate）
- [x] **接頭系統 Phase A**（端板/腹板/底板/螺栓/焊接/肋板）
- [x] **AISC 360-22 驗算**（J3 螺栓+J2 焊接+J10 加勁板）
- [x] **AISC 接頭確認對話框**（選構件→彈出視窗→建議方案→確認繪製）
- [x] **自動編號 Phase B**（C1/B1/BR1/PL1 + 組裝件 A1）
- [x] **施工圖 Phase C**（GA 總裝圖 + 單件圖 → DXF 匯出）
- [x] **NC/DSTV Phase D**（NC1 格式鑽孔+切割 → CNC 對接）
- [x] **IFC 2x3 匯出**（IfcColumn/IfcBeam/IfcPlate/IfcMechanicalFastener）
- [x] **碰撞偵測**（AABB + 螺栓邊距檢查）
- [x] **旋轉工具修正**（實際旋轉物件位置+VCB角度輸入）

### 缺少
- [ ] C 型鋼/L 角鋼截面
- [ ] XYZ 三軸旋轉（目前僅 Y 軸）
- [ ] 耐震接頭（AISC 341 SMF/IMF）
- [ ] 自訂接頭元件

---

## Phase A — 接頭系統 (Connections) ✅ DONE

**目標**: 選取兩個構件 → 自動生成接頭（螺栓+板+焊接）
**狀態**: 已完成（2026-04-02），含 AISC 360-22 驗算 + AISC 對話框

### A.1 資料模型

```rust
/// 接頭定義
pub struct SteelConnection {
    pub id: String,
    pub conn_type: ConnectionType,
    pub member_ids: Vec<String>,     // 參與構件 ID
    pub plates: Vec<ConnectionPlate>,
    pub bolts: Vec<BoltGroup>,
    pub welds: Vec<WeldLine>,
    pub position: [f32; 3],
}

pub enum ConnectionType {
    EndPlate,       // 端板式（梁-柱 剛接）
    ShearTab,       // 腹板式（梁-柱 鉸接）
    FlangePlate,    // 翼板式（梁-梁 續接）
    BasePlate,      // 底板+錨栓
    BracePlate,     // 斜撐接合板
    SplicePlate,    // 拼接板
}

pub struct ConnectionPlate {
    pub width: f32,
    pub height: f32,
    pub thickness: f32,
    pub position: [f32; 3],
    pub rotation: f32,
    pub material: String,
}

pub struct BoltGroup {
    pub bolt_size: BoltSize,     // M16/M20/M22/M24
    pub rows: u32,
    pub cols: u32,
    pub row_spacing: f32,        // mm
    pub col_spacing: f32,
    pub edge_dist: f32,
    pub hole_diameter: f32,
    pub positions: Vec<[f32; 3]>,
}

pub enum BoltSize {
    M16, M20, M22, M24, M27, M30,
}

impl BoltSize {
    pub fn diameter(&self) -> f32;       // 螺栓直徑
    pub fn hole_diameter(&self) -> f32;  // 標準孔徑
    pub fn head_size(&self) -> f32;      // 頭部對邊距離
    pub fn min_spacing(&self) -> f32;    // 最小間距 (2.5d)
    pub fn min_edge(&self) -> f32;       // 最小邊距
}

pub struct WeldLine {
    pub weld_type: WeldType,
    pub size: f32,               // 焊腳尺寸 mm
    pub length: f32,
    pub start: [f32; 3],
    pub end: [f32; 3],
}

pub enum WeldType {
    Fillet,          // 角焊
    FullPenetration, // 全滲透對接焊
    PartialPen,      // 半滲透
}
```

### A.2 UI 工具

| 工具 | 操作方式 | 輸出 |
|------|---------|------|
| 端板接頭 | 選梁+柱 → 自動生成 | 端板+螺栓+肋板 |
| 腹板接頭 | 選梁+柱 → 自動生成 | 剪力板+螺栓 |
| 底板接頭 | 選柱 → 放置底板 | 底板+錨栓+肋板 |
| 螺栓放置 | 選面 → 配置螺栓 | 螺栓組 |
| 焊接標記 | 選邊 → 標記焊接 | 焊接線 |
| 肋板 | 選柱翼板內側 → 加肋板 | 肋板 Box |

### A.3 自動接頭邏輯

```
選取 梁 + 柱 →
  偵測相交位置 →
  判斷接頭類型（剛接/鉸接）→
  計算端板/螺栓尺寸 →
  生成 3D 物件 →
  加入 Scene + 群組
```

### A.4 檔案
- `crates/core/src/steel_connection.rs` — 資料結構
- `app/src/tools/steel_connections.rs` — 接頭工具邏輯
- `app/src/panels/toolbar.rs` — 鋼構工具列 UI

---

## Phase B — 自動編號 + 報表 ✅ DONE

**目標**: 模型完成 → 一鍵自動編號 → 完整報表

### B.1 編號規則
- 柱: C1, C2, C3...
- 梁: B1, B2, B3...
- 斜撐: BR1, BR2...
- 板: PL1, PL2...
- 組裝件: A1, A2...（柱+接頭+板 = 一個組裝件）

### B.2 編號邏輯
- 相同截面+長度 = 相同編號（只標數量）
- 修改構件後增量重編號（不影響已有編號）

### B.3 報表類型
| 報表 | 內容 |
|------|------|
| 材料表 | 截面/長度/數量/單重/小計 |
| 螺栓表 | 尺寸/等級/數量/位置 |
| 焊接表 | 類型/尺寸/長度 |
| 組裝件清單 | 組裝件編號/構件清單/總重 |

---

## Phase C — 施工圖自動生成 ✅ DONE

**目標**: 3D 模型 → 自動產生 2D 施工圖

### C.1 圖面類型
1. **單件圖 (Part drawing)** — 每個構件的加工圖（含孔位/焊接/尺寸）
2. **組裝圖 (Assembly drawing)** — 組裝件的組合圖
3. **GA 總裝圖 (General arrangement)** — 整體結構配置
4. **錨栓佈置圖** — 基礎螺栓平面

### C.2 技術需求
- 3D → 2D 正交投影（正視/側視/上視）
- 隱藏線處理
- 自動標註尺寸/螺栓/焊接符號
- 圖框+標題欄模板
- 輸出到 2D DraftDocument（利用現有 2D 引擎）

### C.3 工作流程
```
3D 鋼構模型
  → 選取構件/組裝件
  → 選擇圖面類型
  → 自動投影到 2D
  → 自動標註
  → 輸出 DXF/PDF
```

---

## Phase D — NC/CNC + IFC ✅ DONE

### D.1 DSTV NC 輸出
- 格式: `.nc1` (Deutscher Stahlbau-Verband)
- 內容: 截面輪廓、孔位、切割線、彎曲
- 對接: CNC 鑽孔機/切割機/焊接機器人

### D.2 IFC 匯出
- 版本: IFC 2x3 / IFC4
- 實體: IfcColumn, IfcBeam, IfcPlate, IfcBoltType, IfcWeld
- 用途: 與 Revit/ArchiCAD/Navisworks 交換

### D.3 碰撞偵測
- 構件 vs 構件 AABB 交叉
- 螺栓 vs 板邊距檢查
- 焊接可達性檢查
- 報告: 碰撞清單 + 高亮顯示

---

## 開發優先序

```
Phase A (接頭)     ████████████████████  DONE ✅  AISC 360-22 + 對話框
Phase B (編號+報表) ████████████████████  DONE ✅  CSV 完整報表
Phase C (施工圖)    ████████████████████  DONE ✅  GA+單件圖 DXF
Phase D (NC+IFC)    ████████████████████  DONE ✅  NC1+IFC2x3+碰撞偵測
```

---

## 鋼構工具列佈局（目標）

```
┌─ 模式: 鋼構 ──────────┐
│ 建模                   │
│ [柱][梁][撐][板]       │
│ [軸][基礎]             │
│                        │
│ 截面                   │
│ [H型鋼  ▼] [方管 ▼]   │
│ [SN400B ▼] 柱高:3500  │
│                        │
│ 接頭                   │
│ [端板][腹板][底板]     │
│ [螺栓][焊接][肋板]     │
│                        │
│ 輸出                   │
│ [料表][編號][施工圖]   │
│ [NC][IFC]              │
│                        │
│ 統計                   │
│ 構件: 24  接頭: 12     │
│ 總重: 4,250 kg         │
│ 螺栓: M20×48 顆       │
└────────────────────────┘
```

---

## CNS/台灣法規參考

| 法規 | 用途 |
|------|------|
| CNS 386 | H 型鋼規格 |
| CNS 2473 | 螺栓（高強度） |
| CNS 4435 | 焊接材料 |
| 鋼構造建築物鋼結構設計技術規範 | 接頭設計 |
| 建築物耐震設計規範 | 韌性接頭要求 |

---

## 核心理念

**Kolibri 鋼構 ≠ Tekla 複製品**

Kolibri 的差異化：
1. **輕量化** — 50MB vs 10GB
2. **AI 原生** — MCP 讓 AI 能操作鋼構（"幫我在 C1 柱加端板接頭"）
3. **台灣 CNS 優先** — 原生支援台灣標準，不需要額外配置
4. **3D+2D+AI 統一** — 建模、出圖、分析在同一個 APP
5. **零依賴** — 不需要授權伺服器、不需要網路
