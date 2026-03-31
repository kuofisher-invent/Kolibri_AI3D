# Kolibri Ai3D — 開發路線圖

最後更新：2026-03-31

## 已完成功能

### 3D 建模核心（SketchUp 風格）
- [x] 推拉 PushPull（SU 風格：click→move→click 確認，面高亮亮藍）
- [x] 線段/弧線/矩形/圓形/方塊/圓柱/球體
- [x] 移動（Move Gizmo XYZ 箭頭）/ 旋轉（15° snap 量角器）/ 縮放
- [x] 偏移 Offset（內縮/外擴 → 自動切換推拉）
- [x] 跟隨路徑 FollowMe
- [x] 群組/元件（雙擊進入編輯、元件同步）
- [x] Undo/Redo（Diff + Full 混合堆疊）
- [x] 捲尺量測 / 持久標註
- [x] 油漆桶（材質瀏覽器 40+ 材質）
- [x] 橡皮擦（點擊刪除 + 拖曳連續刪除）
- [x] Crossing/Window 框選（screen-space AABB 相交測試）

### 渲染
- [x] wgpu PBR Cook-Torrance（Shaded/Wireframe/XRay/HiddenLine/Monochrome/Sketch）
- [x] 雙面渲染（cull_mode: None + shader 背面藍灰色調）
- [x] 選取藍色 tint + 框線
- [x] GPU 自動偵測（HighPerformance）/ FOV 35°

### Snap / 推斷
- [x] 14 種 SnapType + 4 層推斷引擎 + 軸鎖定

### 鋼構模式（feature: steel）
- [x] H 型鋼 CNS 386（41 規格）+ 柱/梁/斜撐/鋼板/軸線/接頭

### 管線模式（feature: piping）
- [x] 7 種管系 CNS 標準 + 管件（彎頭/三通/閥門/法蘭/大小頭）

### BIM / IFC
- [x] IFC 屬性面板 + 自訂屬性集

### 匯入/匯出
- [x] DXF/DWG/SKP/OBJ/STL/glTF/PNG 匯入匯出

### MCP Server
- [x] 26+ 工具 + Web Dashboard + stdio/HTTP 雙通道

### 2D 出圖模式 — ZWCAD 風格（v0.5-v0.6）

**基礎架構：**
- [x] `crates/drafting/` — 2D 實體(19種) + 圖層 + 幾何運算
- [x] `app/src/panels/ribbon.rs` — ZWCAD 深色 Ribbon（4 tab, SVG icons）
- [x] `app/src/panels/draft_canvas.rs` — 2D 深色畫布 + 互動
- [x] `app/src/svg_icons.rs` — resvg SVG→TextureHandle（85 icons）
- [x] Feature flag: `drafting`

**繪圖工具 (10)：**
- [x] 直線/聚合線/弧/圓/矩形/橢圓
- [x] 多邊形（正六邊形預設）/ 雲形線（Catmull-Rom）/ 建構線 / 點

**修改工具 (13)：**
- [x] 移動/複製/旋轉/鏡射/比例/拉伸
- [x] 偏移/陣列/修剪/延伸（框架）
- [x] 圓角/倒角（幾何運算 + 框架）
- [x] 分解（矩形→4線/多段線→線段/多邊形→線段）

**標註工具 (9)：**
- [x] 線性/對齊/角度/半徑/直徑標註
- [x] 連續標註/基線標註
- [x] 文字/引線/填充

**圖塊 (2)：**
- [x] 建立/插入（基礎框架）

**操作體感：**
- [x] ESC 三段式取消（取消繪圖→回 Select→清除選取）
- [x] 右鍵結束/取消
- [x] Shift+click 多選 / 拖曳框選（Window/Crossing）
- [x] Ctrl+C/V/X 剪貼簿
- [x] Delete 刪除
- [x] Hover 高亮（cyan）+ 選取 grip 控制點
- [x] 工具狀態提示 + 游標座標顯示

**UI 元素：**
- [x] ZWCAD 深色主題（全介面深色）
- [x] Drawing1 文件 tab
- [x] 左側屬性/圖層功能列
- [x] 底部模型/配置 tabs + 狀態列（座標/正交/Snap/格線/鎖點/極座標/線寬/Units）
- [x] SVG icons 32px + 文字 11.5pt
- [x] XY 軸指示器 + 十字游標 + 點格線
- [x] F6 快捷鍵切換出圖/建模

---

## 開發中

### v0.7 — 2D CAD 工具完善（對標 ZWCAD 差距 ~184 工具）

**最優先：**
- [ ] Trim/Extend 實際幾何裁剪運算
- [ ] Offset 實際線段/圓/弧偏移
- [ ] Fillet/Chamfer 完整互動流程（選兩圖元→套用）
- [ ] 2D Snap（端點/中點/交點/垂直/切線）+ Object Snap 圓點
- [ ] 特性面板 + 顏色/線型/線寬 3 下拉（隨圖層/自訂）
- [ ] 文字編輯器（MTEXT 完整功能，支援字型/大小/對齊）
- [ ] 圖層管理員對話框（凍結/鎖定/可見/色塊/新增/刪除）
- [ ] 圖塊定義+插入完整流程（含屬性）
- [ ] 符合性質 MATCHPROP
- [ ] 命令列（底部文字輸入，類似 AutoCAD 命令列）

**中優先：**
- [ ] 弧 7 種子模式（3P/SCE/SCA 等）
- [ ] 圓 6 種子模式（2P/3P/TTR 等）
- [ ] 矩形圓角/倒角選項
- [ ] 陣列子模式（矩形/環形/路徑）
- [ ] 多重引線 MLEADER
- [ ] 表格 TABLE + 表格樣式
- [ ] 打斷 BREAK / 接合 JOIN
- [ ] 等分 DIVIDE / 等距 MEASURE
- [ ] 對齊 ALIGN
- [ ] 修訂雲形 REVCLOUD
- [ ] Hatch 實際填充線繪製（平行線/交叉/磚/混凝土）
- [ ] 單行文字 DTEXT
- [ ] 弧長標註 / 座標標註 / 快速標註
- [ ] 標註樣式管理員

**缺少的 Tab：**
- [ ] 插入 Tab（圖塊/XRef/PDF底圖/影像/OLE）
- [ ] 參數化 Tab（幾何約束 12 種 + 尺寸約束 7 種）
- [ ] 版面 Tab（視埠/頁面設定/視埠比例/鎖定）
- [ ] 管理 Tab（CUI 編輯器/載入應用程式/腳本/標準）
- [ ] 協同 Tab（DWG Compare/計數/共用視圖）

### v0.8 — Paper Space 視圖
- [ ] Viewport 內嵌 3D 渲染（render-to-texture）
- [ ] 多視圖配置（平面/立面/剖面/透視）
- [ ] 視圖比例鎖定 + 視埠可拖曳
- [ ] 圖框自動排版

---

## 未來規劃

### v0.9 — PDF / 列印
- [ ] PDF 匯出（printpdf 或 lopdf）
- [ ] DXF 2D 匯出（從 DraftDocument）
- [ ] 列印預覽 / 批次列印
- [ ] 出圖型式管理（CTB/STB）

### v1.0 — BIM 完整化
- [ ] IFC 匯出（ifcxml）
- [ ] 物件屬性資料庫 + 數量估算表 + 明細表產生器

### v1.1 — 正式版
- [ ] 多語言 UI（繁中/英文/日文）
- [ ] 插件系統（WASM）
- [ ] Cloud 協作（CRDTs）
- [ ] 安裝程式（Windows/macOS）

### 鋼構分析外掛
- [ ] AISC W 型鋼規格表
- [ ] 接頭設計/螺栓孔洞/加勁板加工圖
- [ ] 結構分析（荷重組合/挫屈/撓度）

### 效能
- [ ] BVH 空間索引 / GPU instancing / LOD / WebGPU

---

## 統計

| 模組 | 工具數 | 對標 ZWCAD |
|------|:------:|:----------:|
| 繪圖 | 10 | 20 (50%) |
| 修改 | 13 | 19 (68%) |
| 註解 | 9 | 12 (75%) |
| 圖塊 | 2 | 5 (40%) |
| 圖層 | 1 | 10 (10%) |
| 特性 | 0 | 5 (0%) |
| 剪貼簿 | 3 (快捷鍵) | 6 (50%) |
| **總計** | **38** | **~214 (18%)** |

## Feature Flags

| Flag | 描述 | 預設 |
|------|------|------|
| `steel` | 鋼構模式（CNS 386 H 型鋼） | ON |
| `piping` | 管線模式（CNS 管材規格） | ON |
| `drafting` | 2D 出圖模式（ZWCAD Ribbon + Canvas） | ON |

## 架構圖

```
┌─────────────────────────────────────────────┐
│                    APP                       │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌────────────┐ │
│  │Ribbon│ │Canvas│ │Panels│ │ 3D Viewport│ │
│  │(dark)│ │(dark)│ │      │ │  (wgpu)    │ │
│  └──┬───┘ └──┬───┘ └──┬───┘ └─────┬──────┘ │
│     └────┬───┴────┬───┘           │         │
│     EditorState  ViewerState      │         │
│     (41 tools)   (F6 toggle)      │         │
│          └────┬───┘               │         │
│          ┌────┴────┐       ┌──────┴──────┐  │
│          │  Scene   │       │  Renderer   │  │
│          └────┬─────┘       └─────────────┘  │
└───────────────┼──────────────────────────────┘
          ┌─────┼─────┬──────┐
    ┌─────┤     │     │      ├─────┐
┌───┴──┐┌─┴──┐┌┴───┐┌┴────┐┌┴──────┐
│Draft ││ IO ││SKP ││ MCP ││Piping │
│ing   ││    ││    ││     ││       │
│19 ent││    ││    ││26 t ││       │
└──────┘└────┘└────┘└─────┘└───────┘
```
