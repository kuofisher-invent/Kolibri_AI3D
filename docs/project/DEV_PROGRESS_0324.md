# Kolibri_Ai3D 開發進度紀錄

> 日期：2026-03-24
> 程式碼：~19,000 行 Rust，40+ 模組

---

## 今日完成項目

### DXF/DWG/PDF 匯入系統
- ✅ 新版 DXF importer（正確 SECTION 分割，支援 LINE/LWPOLYLINE/POLYLINE/ARC/CIRCLE/SPLINE/ELLIPSE/SOLID/3DFACE/TEXT/MTEXT/DIMENSION/INSERT）
- ✅ 匯入驗證器（ImportValidator：尺度/原點/幾何訊號檢查）
- ✅ 語意偵測器（SemanticDetector：從線段幾何推斷柱/梁/板/軸線）
- ✅ DWG 二進制掃描器（座標提取 + IQR 過濾）
- ✅ PDF 向量路徑提取（FlateDecode 解壓 + 路徑算子解析）
- ✅ 座標自動歸零（偏移 >50m 時正規化到原點）
- ✅ Console debug report（F12 顯示完整解析報告）
- ✅ H 型鋼斷面建模（3 件式：上翼板+腹板+下翼板）

### 操作體感修正
- ✅ Snap 系統全面改為 3D 螢幕空間（不再只用 XZ 地面投影）
- ✅ 任何工具下都顯示 snap 指標（端點🟢/中點🔵/面中心＋/邊上🔴）
- ✅ 量測工具修正（snap 優先於物件 pick，中點可直接量測）
- ✅ 量測起點持續高亮（藍色圓圈 + 「起點」標籤）
- ✅ 油漆桶點擊即塗裝（fallback pick 確保必定生效）
- ✅ AI 提示卡遠離游標（mx+40, my-60 避免擋住點位）
- ✅ Shift 鎖軸（SketchUp 風格）
- ✅ 陣列複製 "3x"（Ctrl+Move 後輸入 3x Enter）
- ✅ 標註工具（D 鍵）+ 文字工具
- ✅ 新增 Dimension/Text 到工具列
- ✅ 檔案對話框支援大寫副檔名（.DXF/.DWG/.PDF）
- ✅ 游標工具小圖標（20px 彩色圓圈跟隨滑鼠）
- ✅ 油漆桶圖標改為 Figma 風格水滴
- ✅ 說明頁面（F1）+ 右上角改善

### 鋼構模式
- ✅ 模式切換（建模 | 鋼構）
- ✅ 6 個鋼構工具（Grid/Column/Beam/Brace/Plate/Connection）
- ✅ H 型鋼柱/梁（3 件式群組）
- ✅ Profile 解析器（H300x150x6x9）
- ✅ 群組選取（點翼板自動選整根構件）

### 碰撞偵測
- ✅ AABB 碰撞系統整合
- ✅ 構件類型規則（梁碰柱=合法，穿透=警告）
- ✅ 移動/建立/推拉時即時碰撞檢查

---

## 🔴 下次優先（DXF 語意偵測調優）

### 問題
BSGS_TEST.dxf（17,902 實體）匯入只偵測到 1 根梁。原因：
1. Grid parser 的 TEXT 匹配邏輯在大量實體中找不準（A/AB/B/C/D 文字存在但沒匹配到軸線）
2. DIMENSION 實體的跨距數值（3040/3800/3040/2950）沒有被用來推斷軸線間距
3. 標高偵測（+4200/3495/455）部分有效但沒完整應用

### 修正方向
```
Phase 1: Grid Parser 強化
├── 1a. 不依賴「TEXT 靠近長線」— 改用 TEXT 位置分群
│   找所有 TEXT 內容為 A/AB/B/C/D → 取其 X 座標作為軸線位置
│   找所有 TEXT 內容為 1/2 → 取其 Y 座標作為軸線位置
├── 1b. 用 DIMENSION 驗證/補強軸線間距
│   DIMENSION 實體有 start/end 座標 + 量測值
│   連續 DIMENSION 鏈 → 累加得到軸線位置
├── 1c. 用圖框偵測（最外圍矩形）排除圖框座標
│   很多 TEXT/LINE 在圖框上，不是建築幾何

Phase 2: Column 偵測強化
├── 2a. INSERT block 名稱匹配（常見柱符號 block 名稱含 "COL"/"柱"/"H"）
├── 2b. 封閉矩形 POLYLINE 在軸線交點 → 柱截面
├── 2c. 重複出現的相同尺寸 block = 柱

Phase 3: Beam 偵測強化
├── 3a. 連接兩個軸線交點的水平線 + 在梁標高（+4200）= 梁
├── 3b. 排除圖框線、標註線、符號線

Phase 4: Elevation 應用
├── 4a. +4200 → top_level, 0 → base_level, 455 → base_plate_height
├── 4b. 柱高 = top_level - base_level = 4200
├── 4c. 梁標高 = top_level = 4200
```

### 測試驗證目標
```
BSGS_TEST.dxf 應該產出：
- X 軸線: A(0), AB(3040), B(6080), C(9880), D(?)
- Y 軸線: 1(0), 2(2950)
- 柱: 8-10 支 H 型鋼，高 4200mm
- 梁: 連接柱頂，跨距 3040/3800/3040
- 柱腳板: 高 455mm
```

---

## 🟡 中期開發方向

### DXF → 3D 完整管線
- 實作 DWG → DXF 自動轉換（整合 ODA File Converter 或 LibreDWG）
- PDF 向量路徑 → 建築語意（不只邊界框）
- 圖面分類（柱位圖/立面圖/剖面圖）
- 多頁 DXF 處理（平面+立面同時解析）

### 操作體感持續改善
- Push/Pull 連接幾何（拉一面影響相鄰面）
- 真正的 Offset（面內邊緣偏移）
- 真正的 Follow Me（沿路徑擠出）
- 效能優化（>100 物件場景）

### 鋼構進階
- Profile 資料庫（H/I/C/L 型鋼標準斷面）
- 接頭細部（端板/剪力板/角鋼）
- BOM 表（材料清單匯出）
- NC 加工數據

---

## 📁 專案文件索引

| 文件 | 位置 |
|------|------|
| 總功能清單 | docs/project/ROADMAP.md |
| SU 差異清單 | docs/project/SU_DIFF.md |
| 待辦事項 | docs/project/TODO_NEXT.md |
| 開發總結 | docs/project/DEV_SUMMARY.md |
| CAD 匯入架構 | docs/project/CAD_IMPORT_PLAN.md |
| DWG/SKP 匯入設計 | docs/project/kolibri_ai_3_d：dwg_skp_file_import.md |
| UX 升級計畫 | docs/project/kolibri_ux_upgrade_plan.md |
| 鋼構工具規劃 | docs/project/LEFT_PANEL_STEEL_TOOLS.md |
| MCP 連接指南 | docs/project/MCP_SETUP.md |
| 外掛系統規劃 | docs/project/PLUGIN_SYSTEM.md |
| 本次進度 | docs/project/DEV_PROGRESS_0324.md |

---

## 桌面捷徑
```
C:\Users\localadmin\Desktop\Kolibri CAD.lnk
→ D:\AI_Design\Kolibri_Ai3D\app\target\release\kolibri-cad.exe
```

## MCP 連接
```json
{
  "mcpServers": {
    "kolibri-ai3d": {
      "command": "D:\\AI_Design\\Kolibri_Ai3D\\app\\target\\release\\kolibri-cad.exe",
      "args": ["--mcp"]
    }
  }
}
```
