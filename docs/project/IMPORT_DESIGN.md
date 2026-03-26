# Kolibri_Ai3D CAD 匯入架構設計

> 更新日期：2026-03-26
> 支援：DWG / DXF / SKP / OBJ / PDF

---

## 架構總覽

```
DWG/DXF/SKP/PDF
    │
    ▼
┌──────────────────────────┐
│  Geometry Parser         │  讀取 LINE/POLYLINE/TEXT/DIMENSION
└──────────┬───────────────┘
           │
┌──────────▼───────────────┐
│  Drawing Classifier      │  判斷圖紙類型（柱位圖/立面圖/裝修圖）
└──────────┬───────────────┘
           │
┌──────────▼───────────────┐
│  Semantic Parsers        │  Grid(軸線) + Steel(柱梁) + Elevation(標高)
└──────────┬───────────────┘
           │
┌──────────▼───────────────┐
│  IR (Intermediate Rep)   │  結構化 JSON（grids/columns/beams/levels）
└──────────┬───────────────┘
           │
┌──────────▼───────────────┐
│  確認面板                 │  使用者檢視 + 手動修正 + [確認建模]
└──────────┬───────────────┘
           │
┌──────────▼───────────────┐
│  Steel Builder           │  create_column / create_beam → Scene
└──────────────────────────┘
```

---

## 模組結構

```
src/
├── cad_import/
│   ├── mod.rs              匯入入口 + 統一調度
│   ├── dxf_importer.rs     DXF 解析器（支援全部實體類型）
│   ├── geometry_parser.rs  幾何解析
│   ├── drawing_classifier.rs 圖紙分類
│   ├── grid_parser.rs      軸線解析（TEXT+DIMENSION→軸線位置）
│   ├── steel_parser.rs     鋼構元件辨識
│   ├── elevation_parser.rs 標高解析
│   ├── semantic_detector.rs 語意偵測（柱/梁/板/軸線）
│   ├── preprocessor.rs     前處理（座標歸零等）
│   ├── import_validator.rs 匯入驗證（尺度/原點/幾何檢查）
│   └── ir.rs               中介資料結構
├── import/
│   ├── import_manager.rs   統一匯入管理器
│   ├── dwg_importer.rs     DWG 匯入
│   ├── dwg_parser.rs       DWG 二進制解析
│   ├── skp_importer.rs     SKP 匯入
│   ├── pdf_parser.rs       PDF 向量路徑提取
│   └── unified_ir.rs       統一 IR（跨格式）
├── dwg_parser/             DWG 底層解析器（R2000–R2025）
│   ├── bitreader.rs        位元級讀取
│   ├── header.rs           檔頭解析
│   ├── sections.rs         區段解析
│   ├── entities.rs         實體解析
│   ├── objects.rs          物件解析
│   ├── decompress.rs       解壓縮
│   └── r2018.rs            R2018+ 格式支援
└── builders/
    └── steel_builder.rs    IR → 3D 鋼構模型
```

---

## DWG vs SKP 差異

| 項目 | DWG | SKP |
|------|-----|-----|
| 本質 | CAD 工程圖 | 3D 模型 |
| 幾何 | line/polyline | face/edge |
| 結構 | layer/block | group/component |
| 語意 | 幾乎沒有 | 部分存在 |

→ 使用 **雙匯入器 + 統一 IR** 解決差異

---

## DWG 解析已知問題與對策

### 核心問題
DWG 二進制格式未被正確解析成幾何資料，導致：
- 匯入後場景空白
- 邊界框異常巨大（誤把非座標當座標）
- 模型遠離原點或異常縮小

### 正確做法
```
DWG binary → 正確解析 Object Map / AcDb entities → 還原 entity 屬性
→ 透過 IR 清洗 → 語意偵測 → 使用者確認 → 3D 建模
```

### 格式支援現況

| 版本 | 代號 | 解析方式 | 狀態 |
|------|------|---------|:----:|
| R2000 | AC1015 | Object Map → Entity | ⚠️ 基礎 |
| R2004 | AC1018 | Section Page Map | ❌ |
| R2007 | AC1021 | 同上 + UTF-8 string | ❌ |
| R2010–R2025 | AC1024–AC1032 | 同上 | ❌ |

### 語意偵測優化方向

```
Phase 1: Grid Parser 強化
├── TEXT 位置分群（A/AB/B/C/D → X 座標作軸線）
├── DIMENSION 鏈驗證軸線間距
└── 圖框偵測排除

Phase 2: Column 偵測
├── INSERT block 名稱匹配
├── 封閉 POLYLINE 在軸線交點 → 柱截面
└── 重複出現的相同尺寸 block

Phase 3: Beam 偵測
├── 連接軸線交點的水平線 + 梁標高
└── 排除圖框線/標註線/符號線
```

---

## AI 的角色

AI 不做解析本身，而做：
- 圖紙分類（柱位圖/立面圖/裝修圖）
- 語意補全與推測缺失資料
- 修正錯誤解析
