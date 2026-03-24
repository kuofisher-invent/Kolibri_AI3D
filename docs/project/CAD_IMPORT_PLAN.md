# Kolibri Ai3D — CAD 匯入解析架構計畫

> 日期：2026-03-24
> 目標：DWG/DXF/PDF → IR → 半自動 3D 建模

---

## 架構總覽

```
DWG/DXF/PDF
    │
    ▼
┌─────────────────────────────────┐
│  Layer 1: Geometry Parser       │  讀取 LINE/POLYLINE/TEXT/DIMENSION
│  → lines, blocks, texts, dims  │
└─────────────┬───────────────────┘
              │
┌─────────────▼───────────────────┐
│  Layer 2: Drawing Classifier    │  判斷圖紙類型
│  → column_layout / elevation    │  (AI 輔助分類)
└─────────────┬───────────────────┘
              │
┌─────────────▼───────────────────┐
│  Layer 3: Semantic Parsers      │
│  ├── Grid Parser (軸線+軸號)    │
│  ├── Steel Parser (柱/梁/基座)  │
│  └── Elevation Parser (標高)    │
└─────────────┬───────────────────┘
              │
┌─────────────▼───────────────────┐
│  IR (Intermediate Rep)          │  乾淨結構化 JSON
│  → grids, columns, beams,      │
│     levels, profiles            │
└─────────────┬───────────────────┘
              │
┌─────────────▼───────────────────┐
│  確認面板 (使用者檢視)           │
│  → 偵測結果 + 手動修正          │
│  → [確認建模] 按鈕              │
└─────────────┬───────────────────┘
              │
┌─────────────▼───────────────────┐
│  Steel Builder                  │
│  → create_column / create_beam  │
│  → Kolibri Scene                │
└─────────────────────────────────┘
```

---

## 模組結構

```
src/
├── cad_import/
│   ├── mod.rs              模組入口
│   ├── geometry_parser.rs  DXF 幾何解析 (LINE/POLYLINE/TEXT/DIMENSION)
│   ├── drawing_classifier.rs 圖紙類型分類
│   ├── grid_parser.rs      軸線解析
│   ├── steel_parser.rs     鋼構元件辨識
│   ├── elevation_parser.rs 標高解析
│   └── ir.rs               中介資料結構 (IR)
├── builders/
│   ├── mod.rs
│   └── steel_builder.rs    IR → Kolibri Scene 建模
```

---

## MVP v1 範圍

1. ✅ 讀 DXF 的 LINE、TEXT、DIMENSION
2. ✅ 找 grid (軸線+軸號)
3. ✅ 找柱位 (grid 交點)
4. ✅ 找立面標高
5. ✅ 自動生成柱+梁線框模型
6. ✅ 使用者確認後轉成 3D

---

## 開發順序

### Phase 1: IR 資料結構 + DXF 幾何解析
### Phase 2: Grid Parser + 圖紙分類
### Phase 3: Steel Parser + Elevation Parser
### Phase 4: Steel Builder + 確認面板 UI
### Phase 5: AI 輔助 (分類/語意/補值)
