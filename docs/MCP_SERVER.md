# Kolibri Ai3D — MCP Server 說明

## 概述

Kolibri Ai3D 提供 [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) 伺服器，讓 AI 助手（Claude Desktop、ChatGPT 等）能直接操控 3D 場景。

## 架構（4 層）

```
┌─────────────────────────────────────────┐
│  Layer 1: protocol.rs                   │  MCP JSON-RPC 2.0 協定型別
├─────────────────────────────────────────┤
│  Layer 2: adapter.rs                    │  Kolibri 工具轉接器（17 tools → Scene API）
├──────────────────┬──────────────────────┤
│  Layer 3a: stdio │  Layer 3b: HTTP/SSE  │  傳輸層（Claude Desktop / ChatGPT）
├──────────────────┴──────────────────────┤
│  Layer 4: test_harness.rs               │  Rust 測試工具
└─────────────────────────────────────────┘
```

原始碼位置：`crates/mcp/src/`

## 三種運行模式

### 1. stdio 模式（Claude Desktop）

```bash
cargo run --bin kolibri-mcp-server
```

- stdin/stdout JSON-RPC 2.0（每行一個 JSON）
- 適合 Claude Desktop、自動化腳本、CI/CD

### 2. HTTP/SSE 模式（ChatGPT）

```bash
cargo run --bin kolibri-mcp-server -- --http       # 預設 port 3001
cargo run --bin kolibri-mcp-server -- --http 8080   # 自訂 port
```

- `POST /mcp` — JSON-RPC 2.0 request → response
- `GET /sse` — Server-Sent Events 即時事件串流
- `GET /health` — 健康檢查（回傳 server info + 物件數量）
- CORS 已啟用，可從瀏覽器/前端直接呼叫

### 3. GUI 內嵌模式

```bash
cargo run -p kolibri-cad -- --mcp
```

APP 內建的舊版 MCP，透過 `mpsc` channel 與 GUI 主執行緒通訊。

### 4. 測試工具

```bash
cargo run --bin kolibri-mcp-test
```

自動執行 initialize → tools/list → create → modify → query → undo/redo → save 完整流程驗證。

## Claude Desktop 設定

**Windows:** `%APPDATA%\Claude\claude_desktop_config.json`
**macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`

```json
{
  "mcpServers": {
    "kolibri-ai3d": {
      "command": "D:/AI_Design/Kolibri_Ai3D/target/debug/kolibri-mcp-server.exe"
    }
  }
}
```

> 路徑請替換為你實際的編譯產出位置。使用新版 `kolibri-mcp-server`（4 層架構）。

## ChatGPT 設定

啟動 HTTP 模式後，在 ChatGPT 的 MCP 設定中填入：

```
Server URL: http://localhost:3001/mcp
```

或使用 SSE endpoint 做即時事件監聽：`http://localhost:3001/sse`

## Web Dashboard

HTTP 模式自帶管理介面，開瀏覽器到 `http://localhost:3001/` 即可使用。

功能：
- **Tool Playground** — 選擇工具、編輯 JSON 參數、即時執行
- **Scene State** — 一鍵查看場景所有物件
- **Event Log** — SSE 即時事件串流（每次工具呼叫都會推送）
- **Server Status** — 物件數量、版本、連線狀態

也可從 Kolibri CAD APP 的頂部列點擊 **MCP** 按鈕啟動 server 並自動開啟 Dashboard。

## 可用工具（17 個）

### 場景查詢

| 工具 | 說明 | 必填參數 |
|------|------|----------|
| `get_scene_state` | 取得全場景狀態（所有物件 ID、尺寸、位置、材質） | — |
| `get_object_info` | 取得單一物件詳細資訊（shape 參數、PBR、材質） | `id` |

### 建立物件

| 工具 | 說明 | 必填參數 | 選填參數 |
|------|------|----------|----------|
| `create_box` | 建立方塊 | `width`, `height`, `depth` (mm) | `name`, `position` [x,y,z], `material` |
| `create_cylinder` | 建立圓柱 | `radius`, `height` (mm) | `name`, `position`, `material` |
| `create_sphere` | 建立球體 | `radius` (mm) | `name`, `position`, `material` |

### 修改物件

| 工具 | 說明 | 必填參數 | 選填參數 |
|------|------|----------|----------|
| `move_object` | 移動物件到絕對位置 | `id`, `position` [x,y,z] mm | — |
| `rotate_object` | Y 軸旋轉（角度制） | `id`, `angle_deg` | — |
| `scale_object` | 縮放物件 | `id`, `factor` [x,y,z] 倍率 | — |
| `set_material` | 設定材質 | `id`, `material` | — |
| `push_pull` | 推拉面 | `id`, `face`, `distance` (mm) | — |
| `duplicate_object` | 複製物件 | `id` | `offset` [x,y,z] mm（預設 [500,0,0]）|

### 場景管理

| 工具 | 說明 | 必填參數 |
|------|------|----------|
| `delete_object` | 刪除物件 | `id` |
| `clear_scene` | 清空全部物件 | — |
| `save_scene` | 儲存場景到 .k3d 檔案 | `path` |
| `load_scene` | 載入 .k3d 場景檔 | `path` |
| `undo` | 撤銷上一步 | — |
| `redo` | 重做 | — |
| `shutdown` | 關閉應用程式 | — |

## 材質名稱對照

`set_material` 和建立物件的 `material` 參數接受以下值：

| 名稱 | 說明 | 名稱 | 說明 |
|------|------|------|------|
| `concrete` | 混凝土（預設） | `marble` | 大理石 |
| `wood` | 木材 | `steel` | 鋼 |
| `glass` | 玻璃 | `aluminum` | 鋁 |
| `metal` | 金屬 | `copper` | 銅 |
| `brick` | 磚 | `gold` | 金 |
| `white` | 白色 | `tile` | 磁磚 |
| `black` | 黑色 | `asphalt` | 柏油 |
| `stone` | 石材 | `grass` | 草地 |
| `plaster` | 灰泥 | | |

## Push/Pull 面名稱

`push_pull` 的 `face` 參數：

| 值 | 方向 |
|----|------|
| `top` | Y+ 上 |
| `bottom` | Y- 下 |
| `front` | Z- 前 |
| `back` | Z+ 後 |
| `left` | X- 左 |
| `right` | X+ 右 |

`distance` 正值 = 向外擴張，負值 = 向內收縮。

## 使用範例

### 建立一棟簡易建築

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"create_box","arguments":{"name":"地板","width":6000,"height":200,"depth":4000,"material":"concrete"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_box","arguments":{"name":"牆壁A","position":[0,200,0],"width":200,"height":3000,"depth":4000,"material":"brick"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_box","arguments":{"name":"牆壁B","position":[5800,200,0],"width":200,"height":3000,"depth":4000,"material":"brick"}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"create_cylinder","arguments":{"name":"柱子","position":[2800,200,1800],"radius":200,"height":3000,"material":"steel"}}}
```

### 查詢 → 修改 → 儲存

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_scene_state","arguments":{}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"move_object","arguments":{"id":"abc12345","position":[1000,0,2000]}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"rotate_object","arguments":{"id":"abc12345","angle_deg":45}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"save_scene","arguments":{"path":"building.k3d"}}}
```

### 撤銷與關閉

```json
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"undo","arguments":{}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"shutdown","arguments":{}}}
```

## 協定細節

- MCP 版本：`2024-11-05`
- Server 名稱：`kolibri-ai3d`
- 支援的 method：
  - `initialize` — 回傳 server info + capabilities
  - `tools/list` — 回傳所有可用工具的 JSON Schema
  - `tools/call` — 執行工具
  - `notifications/initialized` — 確認初始化
  - `resources/list` — 回傳空列表（未使用）
  - `prompts/list` — 回傳空列表（未使用）

## 單位系統

所有幾何參數使用 **毫米（mm）** 為單位：
- `1000` = 1 公尺
- `3000` = 3 公尺（一層樓高度）
- `200` = 20 公分（標準牆厚）
