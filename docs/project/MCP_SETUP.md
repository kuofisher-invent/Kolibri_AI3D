# Kolibri_Ai3D MCP Server 連接指南

> 讓 Claude Desktop 直接控制 Kolibri 3D 場景

---

## 快速設定

### 1. 開啟 Claude Desktop 設定檔

| 系統 | 路徑 |
|------|------|
| **Windows** | `%APPDATA%\Claude\claude_desktop_config.json` |
| **macOS** | `~/Library/Application Support/Claude/claude_desktop_config.json` |
| **Linux** | `~/.config/Claude/claude_desktop_config.json` |

### 2. 加入以下設定

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

### 3. 重啟 Claude Desktop

關閉並重新開啟 Claude Desktop，左下角應出現 🔧 圖示，點開可看到 `kolibri-ai3d`。

---

## 可用工具 (11 個)

### 場景操作

| 工具 | 說明 | 必填參數 |
|------|------|---------|
| `get_scene_state` | 獲取場景所有物件狀態 | 無 |
| `clear_scene` | 清空整個場景 | 無 |
| `save_scene` | 儲存場景 | `path` |
| `load_scene` | 載入場景 | `path` |

### 建立物件

| 工具 | 說明 | 必填參數 | 選填 |
|------|------|---------|------|
| `create_box` | 建立方塊 | `width`, `height`, `depth` (mm) | `name`, `position`, `material` |
| `create_cylinder` | 建立圓柱 | `radius`, `height` (mm) | `name`, `position`, `material` |
| `create_sphere` | 建立球體 | `radius` (mm) | `name`, `position`, `material` |

### 修改物件

| 工具 | 說明 | 必填參數 |
|------|------|---------|
| `move_object` | 移動到指定位置 | `id`, `position` [x,y,z] |
| `set_material` | 設定材質 | `id`, `material` |
| `push_pull` | 推拉面 | `id`, `face`, `distance` (mm) |
| `delete_object` | 刪除物件 | `id` |

---

## 可用材質

```
concrete    混凝土        marble     大理石
wood        木材          steel      鋼
glass       玻璃          aluminum   鋁
metal       金屬          copper     銅
brick       紅磚          gold       金
white       白色          tile       磁磚
asphalt     柏油路        grass      草地
```

## 可推拉面

```
top     上面        bottom  下面
front   前面        back    後面
left    左面        right   右面
```

---

## 使用範例

對 Claude Desktop 說：

### 範例 1：建一個客廳
```
幫我建一個 5x4 公尺的客廳，牆高 2.8 米，
北牆和東牆用磚，地板用木材。
```

Claude 會呼叫：
```
→ create_box(name="地板", width=5000, height=100, depth=4000, material="wood")
→ create_box(name="北牆", width=5000, height=2800, depth=200, material="brick")
→ create_box(name="東牆", position=[4800,0,0], width=200, height=2800, depth=4000, material="brick")
```

### 範例 2：修改場景
```
把北牆的材質改成大理石，然後把東牆往右移 500mm。
```

Claude 會呼叫：
```
→ get_scene_state()  (先感知場景)
→ set_material(id="xxx", material="marble")
→ move_object(id="yyy", position=[5300, 0, 0])
```

### 範例 3：開窗
```
在北牆上開一個 1.5m x 1.2m 的窗，離地 800mm。
```

Claude 會呼叫：
```
→ create_box(name="窗洞", position=[1500,800,0], width=1500, height=1200, depth=220, material="glass")
```

---

## 審計日誌

所有 Claude 的操作都會記錄在 AI 審計日誌中：
- 記錄欄位：時間、操作者(Claude)、動作、影響物件
- GUI 中可在右側面板「記錄」分頁查看
- 可匯出為 `ai_log.json`

---

## 技術規格

| 項目 | 值 |
|------|---|
| 協議 | MCP (Model Context Protocol) |
| 傳輸 | stdio (JSON-RPC 2.0，換行分隔) |
| 編碼 | UTF-8 |
| 協議版本 | 2024-11-05 |
| 單位 | 毫米 (mm) |
| 座標系 | 右手系，Y 軸朝上 |
| 二進制 | `kolibri-cad.exe --mcp` |
| 檔案格式 | `.k3d` (JSON) |

---

## 故障排除

| 問題 | 解決 |
|------|------|
| Claude Desktop 看不到工具 | 確認路徑正確，重啟 Claude Desktop |
| 回應 "GUI not responding" | 使用 `--mcp` 模式（獨立場景，不需 GUI） |
| 物件 ID 找不到 | 先呼叫 `get_scene_state` 獲取正確 ID |
| 材質名稱錯誤 | 參考上方材質列表，使用英文名 |
