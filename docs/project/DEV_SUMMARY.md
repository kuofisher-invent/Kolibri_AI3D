# Kolibri_Ai3D 開發總結

> 日期：2026-03-23
> 從零到可用的 SketchUp-like 3D CAD 應用，一天完成

---

## 專案概況

| 項目 | 值 |
|------|---|
| 語言 | Rust 2021 |
| 渲染 | wgpu + WGSL shader |
| UI | egui (eframe 0.28) |
| 原始碼 | 23 個模組，10,970 行 |
| 二進制 | kolibri-cad.exe（~12 MB） |
| 平台 | Windows 11（GPU: NVIDIA RTX 4060 Laptop） |

---

## 模組結構

```
src/
├── main.rs          (154)   入口 + 圖標 + --mcp 分支
├── app.rs          (1596)   核心 struct + 主題 + update()
├── tools.rs        (1763)   工具互動 + 點擊/拖曳處理
├── panels.rs       (1214)   UI 面板（toolbar + 右側 + 場景）
├── renderer.rs     (1447)   wgpu 渲染管線 + shader + 網格生成
├── scene.rs         (549)   場景資料模型 + undo/redo + 存取
├── mcp_server.rs    (466)   MCP JSON-RPC Server（11 工具）
├── test_bridge.rs   (511)   檔案式測試入口
├── snap.rs          (449)   推斷/捕捉系統
├── icons.rs         (406)   22 個手繪向量圖標
├── halfedge.rs      (396)   半邊網格資料結構
├── camera.rs        (188)   軌道相機 + 行走 + 正交投影
├── menu.rs          (174)   選單列 + 右鍵選單
├── preview.rs       (275)   繪圖預覽 + 縮放手柄
├── dimensions.rs    (138)   尺寸標註 2D 覆蓋
├── obj_io.rs        (333)   OBJ 匯出入
├── gltf_io.rs       (188)   GLTF 匯出
├── stl_io.rs        (141)   STL 匯出入
├── dxf_io.rs        (137)   DXF 匯出入
├── csg.rs           (142)   布林運算（聯集/差集/交集）
├── measure.rs        (86)   面積/體積/重量計算
├── ai_log.rs         (87)   AI 審計日誌
└── file_io.rs       (130)   檔案存取 + 自動儲存
```

---

## 功能清單

### 繪圖工具（22 個）
- 選取、移動（Ctrl 切軸 X/Y/Z）、旋轉、縮放（等比+非等比+精確輸入）
- 線段、弧線、矩形、圓形
- 方塊、圓柱、球體（SketchUp 風格互動式建立）
- 推拉（面級別，六面獨立，螢幕投影法線方向）
- 偏移、跟隨、量測、油漆桶
- 環繞、平移、全部顯示
- 群組、元件、橡皮擦

### 渲染
- 天空漸層背景（可切換明/暗）
- 地面格線 + XYZ 軸線
- 方向光 + 天光環境光
- 邊線描繪（精確幾何邊線 + shader 輔助）
- 面正反面不同顏色
- 平滑法線（圓柱/球體）
- 5 種顯示模式（著色/線框/X光/隱藏線/單色）
- 程序化材質紋理（磚/木/金屬/混凝土/大理石/磁磚/柏油/草地）
- 選取高亮（藍色邊框 + 軸向面色）
- 尺寸標註 2D 覆蓋

### 材質系統
- 29 種預設材質（7 大類：石材/木材/金屬/磚瓦/玻璃/路面/其他）
- 自訂 RGBA 顏色
- PBR 參數（粗糙度/金屬感）
- 材質預覽球
- 油漆桶工具套用

### 選取互動
- 點擊選取（任何工具下）
- 懸停高亮
- 面級別選取（推拉用，軸向顏色指示）
- 多選（Shift+Click）
- 框選（Rubber Band）
- 雙擊編輯群組（隔離模式）

### 組織管理
- 群組（選取多物件→建立群組→雙擊進入）
- 元件實例（改一個同步全部）
- 圖層/標籤（自訂標籤 + 可見性切換）
- 場景大綱（物件列表 + 點擊選取 + 刪除）

### 相機
- 軌道旋轉（中鍵拖曳，匹配 SU 方向）
- 平移（Shift+中鍵）
- 游標中心縮放（滾輪）
- WASD 行走模式
- 透視/正交切換
- 標準視圖（前/後/左/右/上/等角）
- 場景視角儲存/恢復

### 推斷/捕捉
- 端點/中點/原點捕捉
- 軸向引導線（RGB 彩色虛線）
- 面上繪圖（Line/Arc 吸附物件面）
- 500mm 格線捕捉
- 文字提示標籤

### 編輯
- Undo/Redo（50 步歷史）
- 複製移動（Ctrl+Move）
- 刪除（Delete 鍵 / 橡皮擦拖曳連刪）
- 雙擊推拉重複上次距離
- 精確尺寸輸入（底部輸入框 + Enter）

### 檔案 I/O
- 儲存/載入 .k3d（JSON）
- 匯出入 OBJ / STL / GLTF / DXF
- 匯出 PNG / JPG 截圖
- 自動儲存（60 秒）
- 最近檔案列表
- 拖放開啟
- 未儲存提示

### UI（Figma 風格）
- 淺色毛玻璃面板
- 頂部品牌列（Logo + 選單 + 專案名 + Undo/Redo）
- 左側合併面板（工具列 + 場景/圖層/快速操作/捕捉）
- 右側分頁面板（設計/屬性/場景/輸出）
- 浮動視角切換按鈕（透視/正視/俯視/左視/線框/著色）
- 浮動工具資訊卡
- 浮動方向鍵盤
- 浮動座標晶片（X/Y/Z/Snap/Units）
- Selection Summary 統計卡片
- 材質色票網格（28 色）
- 工具圖標 48px + 快捷鍵標示
- 游標回饋（工具不同游標不同）
- 狀態列（工具說明 + 游標座標 + 物件數）

### AI 整合
- MCP Server 內建（`--mcp` stdio 模式，11 個工具）
- Claude Desktop 直接連接
- AI 審計日誌（署名 + 時間 + 動作 + 物件）
- 測試入口（JSON 指令 → 截圖/狀態查詢）

### 布林運算
- 聯集（A+B → 合併邊界框）
- 差集（A-B → 切割成最多 6 塊）
- 交集（A∩B → 保留重疊部分）

---

## 明日待辦（優先順序）

### 🔴 高優先
1. 線條/邊選取高亮（free_mesh pick 機制）
2. 推拉方向全面驗證
3. 推拉即時距離顯示驗證

### 🟡 中優先
4. 旋轉工具量角器模式
5. 橡皮擦刪除 free_mesh 邊
6. 捲尺工具完善（構造線 + 物件間距）
7. 面上繪圖完善

### 🟢 低優先
8. 多邊形/徒手畫工具
9. 線段切割相交面
10. MSAA 反鋸齒
11. PDF 匯出
12. 圖片參考底圖
13. 材質色票調色（偏建築色系）

---

## 文件索引

| 文件 | 說明 |
|------|------|
| [ROADMAP.md](ROADMAP.md) | 總功能清單（96 項，91.7% 完成） |
| [SU_DIFF.md](SU_DIFF.md) | 與 SketchUp 差異（20 項，前 3 波已完成） |
| [TODO_NEXT.md](TODO_NEXT.md) | 詳細待辦清單（17 項） |
| [PLUGIN_SYSTEM.md](PLUGIN_SYSTEM.md) | 外掛系統規劃（Lua/WASM/HTTP） |
| [PLUGIN_API.md](PLUGIN_API.md) | 外掛 HTTP API 規格 |
| [MCP_SETUP.md](MCP_SETUP.md) | Claude Desktop MCP 連接指南 |
| [DEV_SUMMARY.md](DEV_SUMMARY.md) | 本文件 |

---

## 桌面捷徑

```
位置：C:\Users\localadmin\Desktop\Kolibri CAD.lnk
目標：D:\AI_Design\Kolibri_Ai3D\app\target\release\kolibri-cad.exe
工作目錄：D:\AI_Design\Kolibri_Ai3D\app
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
