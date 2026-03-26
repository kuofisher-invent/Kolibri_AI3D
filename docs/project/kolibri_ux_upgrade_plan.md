# Kolibri Ai3D UX 升級計畫 v2.0

> 更新日期：2026-03-26
> 定位：**SketchUp 操作體驗 + Pascal 建築流程 + AI 智能輔助 + Rust 桌面效能**

---

## 核心理念

SketchUp 的成功不是功能，而是：**操作流暢 + 即時回饋 + 推論穩定**

- 人主導操作（滑鼠體感），AI 輔助決策
- 即時回饋（數值 / 推論 / 吸附）
- 建築用戶要「非技術」流程（向 Pascal 學習）

---

## 第一階段：體感強化 ✅ 已完成

| 項目 | 狀態 |
|------|:----:|
| 游標旁即時數值顯示（Move/PushPull/Draw） | ✅ |
| 任意工具皆可選取（不需切換 Select） | ✅ |
| Push/Pull 雙擊重複 + 虛線參考 | ✅ |
| Ctrl+Move 複製 + 陣列複製 "3x" | ✅ |

---

## 第二階段：Inference 強化 ✅ 已完成

| 項目 | 狀態 |
|------|:----:|
| 軸向鎖定提示（On Red/Green/Blue Axis） | ✅ |
| 推論 + 數值整合（「端點 — 2500 mm」） | ✅ |
| 面吸附（Line/Arc 吸附物件面） | ✅ |
| Shift 鎖軸（SketchUp 風格） | ✅ |
| Inference 2.0 評分管線 | ✅ |

---

## 第三階段：操作回饋 ✅ 大部分完成

| 項目 | 狀態 |
|------|:----:|
| Hover 高亮（Object 級別） | ✅ |
| Edge/Face 級別高亮 | ⚠️ |
| 游標圖示（十字/移動/手/鉛筆/禁止） | ✅ |
| 面正反面不同色（白/藍） | ✅ |
| 6 種渲染模式切換 | ✅ |

---

## 第四階段：AI 輔助層

| 項目 | 狀態 |
|------|:----:|
| AI 推薦工具 + 操作建議 | ⚠️ |
| AI 數值補全 + 推論修正 | ⚠️ |
| 文字提示生成物件（本地 LLM） | ❌ |
| 圖片轉 3D（image-to-3D） | ❌ |
| 自動優化 mesh / 自動生成材質 | ❌ |

桌面優勢：可跑本地大型 AI 模型，不需依賴雲端（隱私更好、速度更快）。

---

## 第五階段：Hybrid 工作流

| 模式 | 說明 |
|------|------|
| 自動（AI） | CAD → 語意辨識 → 初步建模（80%） |
| 手動（使用者） | 修正 → 細節建模 → 結構調整 |
| 混合模式（未來） | 程式碼 + 視覺化編輯（類似 Grasshopper / KCL） |

---

## 第六階段：向 Pascal 學習 — 建築專用流程

### Smart Walls / 參數化建模
讓使用者像 Pascal 一樣快速畫牆、自動生成房間、調整厚度/高度。
建築用戶需要「非技術」流程。

| 項目 | 說明 | 狀態 |
|------|------|:----:|
| Wall 工具 | 智能牆體（厚度/高度/自動接合） | ❌ |
| Slab 工具 | 樓板（自動偵測封閉區域） | ❌ |
| Roof 工具 | 屋頂生成 | ❌ |
| Door/Window 工具 | 自動切洞 + 參數化開口 | ❌ |

### Snap 系統強化

| 項目 | 狀態 |
|------|:----:|
| Grid Snap | ✅ |
| Vertex/Endpoint Snap | ✅ |
| Edge Snap | ✅ |
| Midpoint Snap | ✅ |
| Angle Snap（15°/30°/45°/90°） | ❌ |
| 即時數值提示 | ✅ |

### 視角控制優化

| 項目 | 狀態 |
|------|:----:|
| Orbit/Pan/Zoom 快捷鍵 | ✅ |
| Top/Front/Side 快捷按鈕 | ✅ |
| Fit to View（Zoom Extents） | ✅ |
| 平滑動畫過渡 | ❌ |

---

## 第七階段：Rust 特有優勢發揮

### 效能優化
| 項目 | 說明 | 狀態 |
|------|------|:----:|
| wgpu 自訂渲染管線 | 大型場景明顯勝過 Web 版 | ✅ |
| GPU 實例化 + 視錐剔除 | >500 物件場景 | ❌ |
| truck B-Rep kernel | Rust 精確實體建模（Boolean） | ❌ |
| MSAA 4x 反鋸齒 | 視覺品質 | ❌ |

### 材質與顯示
| 項目 | 說明 | 狀態 |
|------|------|:----:|
| 29 種建築材質 | 程序化紋理 | ✅ |
| PBR 材質（roughness/metallic） | 基礎參數 | ✅ |
| Normal Map + 即時預覽燈光 | 進階 PBR | ❌ |
| 紋理貼圖 UV | 圖片材質 | ❌ |

### 匯入/匯出
| 格式 | 匯入 | 匯出 | 備註 |
|------|:----:|:----:|------|
| .k3d（原生） | ✅ | ✅ | JSON 格式 |
| .obj | ✅ | ✅ | |
| .stl | ✅ | ✅ | |
| .gltf/.glb | — | ✅ | 與 Pascal 相容 |
| .dxf | ✅ | ✅ | |
| .dwg | ✅ | ❌ | 二進制解析 |
| .skp | ⚠️ | — | 部分支援 |
| .pdf | ✅ | ❌ | 向量路徑提取 |
| .fbx | ❌ | ❌ | 需第三方 lib |
| .step | ❌ | ❌ | 精確 CAD |
| .ifc | ❌ | ❌ | BIM 業界標準 |

### 測量工具
| 項目 | 狀態 |
|------|:----:|
| 距離測量 | ✅ |
| 面積計算 | ⚠️ |
| 體積/重量估算 | ⚠️ |

---

## 快捷鍵（業界標準）

| 快捷鍵 | 功能 | 狀態 |
|--------|------|:----:|
| Space | Select | ✅ |
| M | Move | ✅ |
| Q | Rotate | ✅ |
| S | Scale | ✅ |
| L | Line | ✅ |
| A | Arc | ✅ |
| R | Rectangle | ✅ |
| C | Circle | ✅ |
| P | Push/Pull | ✅ |
| B | Paint Bucket | ✅ |
| T | Tape Measure | ✅ |
| D | Dimension | ✅ |
| E | Eraser | ✅ |
| G | Group | ✅ |
| Ctrl+Z/Y | Undo/Redo | ✅ |
| Ctrl+S/O | Save/Open | ✅ |
| Ctrl+A | Select All | ✅ |
| F1 | Help | ✅ |
| 1/2/3/5 | Views/Ortho | ✅ |
| Z | Zoom Extents | ✅ |

---

## 差異化策略

### vs Pascal Editor（Web/WebGPU）
| 面向 | Pascal | Kolibri | 優勢方 |
|------|--------|---------|:------:|
| 平台 | 瀏覽器 | 桌面原生 | 各有 |
| 效能 | WebGPU 限制 | wgpu 原生 GPU | **Kolibri** |
| 離線使用 | 需網路 | 完全離線 | **Kolibri** |
| Smart Walls | ✅ | ❌（規劃中） | Pascal |
| AI 整合 | ❌ | ✅ MCP + 本地 LLM | **Kolibri** |
| CAD 匯入 | ❌ | ✅ DWG/DXF/PDF | **Kolibri** |
| 鋼構模式 | ❌ | ✅ Tekla-like | **Kolibri** |
| 精確 B-Rep | ❌ | ❌（truck 規劃中） | 待開發 |

### Kolibri 獨有優勢
- 本地 AI（隱私、速度）
- CAD 語意匯入（DWG → 3D 自動建模）
- 鋼構專業模式（H 型鋼、碰撞偵測）
- Rust 穩定性（無記憶體洩漏、長時間運行）
- 跨平台潛力（wgpu + egui = Win/Mac/Linux）

---

## 實務建議

- **專案格式**：.kolibri（含歷史記錄 + 自動備份）→ 目前 .k3d 已實作
- **效能測試**：低階 GPU + 高解析場景壓力測試
- **使用者回饋**：建築 vs 產品設計兩群分開收集
- **社群策略**：考慮核心開源 → 吸引 Rust 社群貢獻 → 免費版 + Pro 版（進階 AI/匯出）

---

## 優先順序總覽

| 優先級 | 項目 | ROI |
|:------:|------|:---:|
| 1 | Smart Walls（建築流程） | 極高 |
| 2 | Angle Snap + 視角動畫 | 高 |
| 3 | PBR Normal Map + 紋理 UV | 高 |
| 4 | GPU 實例化 + 效能優化 | 高 |
| 5 | truck B-Rep 精確建模 | 中 |
| 6 | 文字/圖片 → 3D（AI） | 中 |
| 7 | IFC/STEP 匯出 | 中 |
| 8 | 混合模式（程式碼+視覺化） | 低 |
