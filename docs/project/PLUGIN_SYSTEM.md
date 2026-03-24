# Kolibri_Ai3D 外掛系統規劃

> 建立日期：2026-03-23
> 狀態：規劃中

---

## 一、目標

仿照 SketchUp Extension 系統，讓第三方開發者（包含 AI）可以擴展 Kolibri 功能。

---

## 二、SketchUp 外掛系統參考

| 功能 | SU 做法 | Kolibri 規劃 |
|------|---------|-------------|
| 外掛語言 | Ruby | Lua / WASM / HTTP API |
| 外掛格式 | .rbz (ZIP) | .k3dp (ZIP) |
| 安裝方式 | 選單→擴充功能→安裝 | 選單→外掛→安裝 |
| 外掛商店 | Extension Warehouse | 本地資料夾 + 未來線上商店 |
| API 範圍 | 完整建模 API | JSON-RPC Tool 呼叫 |
| UI 擴展 | WebDialog / HtmlDialog | egui 面板（受限）或 Web 面板 |

---

## 三、架構設計

```
┌─────────────────────────────────────────────────┐
│                Kolibri_Ai3D App                   │
│                                                   │
│  ┌───────────────────────────────────────────┐   │
│  │          Plugin Manager (內建)              │   │
│  │                                             │   │
│  │  ┌─────────┐  ┌─────────┐  ┌───────────┐ │   │
│  │  │ Loader  │  │ Sandbox │  │ Plugin UI │ │   │
│  │  │ 載入外掛 │  │ 安全沙盒 │  │ 外掛面板  │ │   │
│  │  └─────────┘  └─────────┘  └───────────┘ │   │
│  └──────────────────┬────────────────────────┘   │
│                     │ Plugin API (JSON-RPC)        │
│                     ↓                              │
│  ┌──────────────────────────────────────────────┐│
│  │              Plugin Host API                  ││
│  │                                               ││
│  │  場景操作 | 相機控制 | UI 註冊 | 事件監聽     ││
│  │  檔案存取 | 材質管理 | 選取查詢 | 工具註冊     ││
│  └──────────────────────────────────────────────┘│
└──────────────────────────────────────────────────┘
         ↑              ↑              ↑
    ┌────────┐    ┌────────┐    ┌────────┐
    │ Lua 外掛│    │ WASM 外掛│   │HTTP 外掛│
    │ (.lua) │    │ (.wasm)│    │(遠端AI) │
    └────────┘    └────────┘    └────────┘
```

---

## 四、外掛類型

### Type A：腳本外掛（Lua）
- 嵌入 Lua 直譯器（`mlua` crate）
- 外掛是 `.lua` 檔案
- 可直接呼叫 Host API
- 適合：參數化建模、批次操作、自訂工具

```lua
-- 範例：牆壁生成器
function create_wall(length, height, thickness)
    local id = kolibri.create_box({
        name = "Wall",
        width = length,
        height = height,
        depth = thickness,
        material = "concrete"
    })
    kolibri.log("建立牆壁 " .. length .. "mm")
    return id
end

-- 註冊為工具
kolibri.register_tool({
    name = "牆壁工具",
    icon = "wall",
    on_click = function(pos)
        create_wall(3000, 2800, 200)
    end
})
```

### Type B：WASM 外掛
- 用任何語言編寫，編譯為 WASM
- 在沙盒中執行（安全）
- 透過 Host API 橋接
- 適合：複雜演算法、幾何核心、第三方 library

### Type C：HTTP 外掛（AI 專用）
- 外部程序透過 HTTP 連接
- JSON-RPC 2.0 協議
- 需要 API Key 認證
- 適合：AI 代理、遠端服務、跨平台

---

## 五、Plugin Host API（外掛可呼叫的功能）

### 5.1 場景操作
```
create_box(name, position, width, height, depth, material) → id
create_cylinder(name, position, radius, height, material) → id
create_sphere(name, position, radius, material) → id
delete_object(id) → bool
move_object(id, position)
set_material(id, material)
push_pull(id, face, distance)
get_scene_state() → {objects, camera, ...}
get_object(id) → {shape, position, material, ...}
```

### 5.2 選取操作
```
get_selection() → [id, ...]
set_selection(ids)
pick_at(screen_x, screen_y) → id
```

### 5.3 相機控制
```
set_camera(target, distance, yaw, pitch)
get_camera() → {target, distance, yaw, pitch}
zoom_extents()
set_view(name)  -- "front", "top", "iso"
```

### 5.4 UI 註冊
```
register_tool(name, icon, callbacks)
register_panel(name, html_content)
add_menu_item(menu, name, callback)
show_dialog(title, html, width, height)
```

### 5.5 事件監聽
```
on_selection_change(callback)
on_scene_change(callback)
on_tool_activate(callback)
on_object_created(callback)
on_object_deleted(callback)
```

### 5.6 審計日誌
```
log(action, details, objects)  -- 自動附帶外掛署名
```

---

## 六、外掛封裝格式 (.k3dp)

```
my_plugin.k3dp (ZIP 格式)
├── manifest.json        # 外掛描述
├── main.lua             # 或 main.wasm
├── icon.png             # 工具列圖標 (32x32)
├── README.md            # 說明文件
└── assets/              # 其他資源
    ├── textures/
    └── models/
```

### manifest.json
```json
{
  "id": "com.example.wall-generator",
  "name": "牆壁生成器",
  "version": "1.0.0",
  "author": "開發者名稱",
  "description": "快速生成各種牆壁",
  "type": "lua",
  "entry": "main.lua",
  "icon": "icon.png",
  "api_version": "1.0",
  "permissions": [
    "scene:write",
    "camera:read",
    "ui:toolbar"
  ],
  "menu": "工具/牆壁生成器"
}
```

---

## 七、安全性

| 層級 | 說明 |
|------|------|
| 權限系統 | 外掛需宣告使用的 API 權限 |
| 沙盒 | WASM 天然沙盒；Lua 限制 IO |
| API Key | HTTP 外掛需要認證 |
| 審計日誌 | 所有操作自動記錄外掛署名 |
| 使用者確認 | 安裝時提示權限列表 |

---

## 八、外掛管理 UI

```
┌─────────────────────────────────┐
│ 外掛管理                    ✕  │
├─────────────────────────────────┤
│ ✅ 牆壁生成器 v1.0.0          │
│    作者: Developer  [停用] [刪除]│
│                                 │
│ ✅ AI 助手 (Claude) v1.0.0     │
│    內建 MCP Server   [設定]     │
│                                 │
│ ☐ 結構分析 v0.5.0              │
│    作者: Engineer  [啟用] [刪除] │
│                                 │
│ [安裝外掛...]  [開啟外掛資料夾] │
└─────────────────────────────────┘
```

---

## 九、開發順序

| 階段 | 項目 | 說明 | 狀態 |
|:----:|------|------|:----:|
| 1 | Plugin Manager 骨架 | 載入/解除安裝 manifest | ❌ |
| 2 | Lua 直譯器整合 | mlua crate + Host API 橋接 | ❌ |
| 3 | 範例外掛 | 牆壁生成器 / 樓梯工具 | ❌ |
| 4 | 外掛管理 UI | 選單 + 管理視窗 | ❌ |
| 5 | WASM 支援 | wasmtime crate 整合 | ❌ |
| 6 | HTTP 外掛 | localhost API + API Key | ❌ |
| 7 | 外掛商店 | 線上瀏覽/下載 | ❌ |

---

## 十、與 AI 系統整合

| AI 連接方式 | 適用場景 |
|------------|---------|
| **MCP stdio（內建）** | Claude Desktop 直接控制場景 |
| **HTTP Plugin** | GPT-4o / Gemini / 本地 LLM |
| **Lua Script** | AI 生成 Lua 腳本後載入執行 |
| **多 AI 討論** | 共享場景 + 審計日誌 + 訊息通道 |

### 多 AI 協作流程
```
1. 使用者: "幫我設計一個 3 房 2 廳的公寓"
2. Claude (MCP): 生成基本平面配置
3. GPT-4o (HTTP): 審查動線建議修改
4. Claude (MCP): 套用修改、加入家具
5. 審計日誌記錄每個 AI 的貢獻
6. 使用者: 最終確認
```
