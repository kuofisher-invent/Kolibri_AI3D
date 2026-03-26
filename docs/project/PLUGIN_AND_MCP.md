# Kolibri_Ai3D 外掛系統 + MCP 連接

> 更新日期：2026-03-26

---

## MCP Server 快速設定

### Claude Desktop 設定

在 `%APPDATA%\Claude\claude_desktop_config.json` 加入：

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

重啟 Claude Desktop 後左下角出現工具圖示。

### 可用工具（11 個）

| 工具 | 說明 | 必填參數 |
|------|------|---------|
| `get_scene_state` | 獲取場景狀態 | 無 |
| `clear_scene` | 清空場景 | 無 |
| `save_scene` | 儲存場景 | `path` |
| `load_scene` | 載入場景 | `path` |
| `create_box` | 建立方塊 | `width`, `height`, `depth` (mm) |
| `create_cylinder` | 建立圓柱 | `radius`, `height` (mm) |
| `create_sphere` | 建立球體 | `radius` (mm) |
| `move_object` | 移動物件 | `id`, `position` [x,y,z] |
| `set_material` | 設定材質 | `id`, `material` |
| `push_pull` | 推拉面 | `id`, `face`, `distance` |
| `delete_object` | 刪除物件 | `id` |

### 可用材質
`concrete` `wood` `glass` `metal` `brick` `white` `black` `stone` `marble` `steel` `aluminum` `copper` `gold` `tile` `asphalt` `grass` `plaster`

### 可推拉面
`top` `bottom` `front` `back` `left` `right`

### 使用範例

```
幫我建一個 5x4 公尺的客廳，牆高 2.8 米，北牆和東牆用磚，地板用木材。
```

Claude 會呼叫 `create_box` 建立地板和牆壁。

---

## HTTP Plugin API

外部 AI 可透過 HTTP 連接（`POST http://localhost:9901/rpc`，JSON-RPC 2.0）。

### 認證
`X-Plugin-Token: <token>`（啟動時生成，從 Settings > Plugin Token 複製）

### Actor 識別
所有修改操作需附帶 `actor` 物件用於審計日誌：
```json
{ "actor": { "name": "Claude", "model": "claude-sonnet-4-20250514", "session_id": "abc" } }
```

### 錯誤碼
| Code | Meaning |
|------|---------|
| -32601 | Method not found |
| -32602 | Invalid params |
| -32001 | Object not found |
| -32002 | Auth error |

---

## 外掛系統架構（規劃中）

### 三種外掛類型

| 類型 | 語言 | 適用場景 |
|------|------|---------|
| **Lua 腳本** | Lua（mlua crate）| 參數化建模、批次操作、自訂工具 |
| **WASM** | 任何語言 → WASM | 複雜演算法、第三方 library |
| **HTTP** | 任何語言 | AI 代理、遠端服務 |

### 外掛封裝（.k3dp）
```
my_plugin.k3dp (ZIP)
├── manifest.json    # id, name, version, type, permissions
├── main.lua         # 或 main.wasm
├── icon.png         # 32x32
└── assets/
```

### Host API（外掛可呼叫）
- **場景操作**：create_box/cylinder/sphere, delete, move, set_material, push_pull, get_scene_state
- **選取操作**：get_selection, set_selection, pick_at
- **相機控制**：set_camera, get_camera, zoom_extents, set_view
- **UI 註冊**：register_tool, register_panel, add_menu_item
- **事件監聽**：on_selection_change, on_scene_change, on_object_created
- **審計日誌**：log（自動附帶外掛署名）

### 安全性
權限宣告制、WASM 沙盒、HTTP API Key、審計日誌全記錄

### AI 多代理協作
```
使用者 → Claude (MCP) 生成平面 → GPT-4o (HTTP) 審查動線
→ Claude 套用修改 → 審計日誌記錄每個 AI 的貢獻
```

### 開發順序
1. Plugin Manager 骨架 → 2. Lua 整合 → 3. 範例外掛 → 4. 管理 UI → 5. WASM → 6. HTTP → 7. 線上商店

---

## 技術規格

| 項目 | 值 |
|------|---|
| MCP 協議 | stdio, JSON-RPC 2.0 |
| HTTP API | localhost:9901 |
| 編碼 | UTF-8 |
| 單位 | 毫米 (mm) |
| 座標系 | 右手系，Y 軸朝上 |
| 檔案格式 | .k3d (JSON) |

### 故障排除
| 問題 | 解決 |
|------|------|
| Claude Desktop 看不到工具 | 確認路徑正確，重啟 |
| 物件 ID 找不到 | 先 `get_scene_state` 獲取 ID |
| 材質名稱錯誤 | 使用上方英文名 |
