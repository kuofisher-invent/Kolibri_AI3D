# CAD 3D MCP Server

跨平台 AI 驅動 3D 建模工具的 **MCP Server 核心**，讓 Claude 桌面版能雙向控制你的 3D 場景。

## 架構

```
Claude Desktop ←──MCP stdio──→ cad-mcp-server (Rust)
                                      ↕
                               CadScene (in-memory B-Rep)
                                      ↕
                         (下一步) wgpu 渲染視窗
```

## 快速開始

```bash
chmod +x setup.sh
./setup.sh
```

## 可用 MCP Tools

| Tool | 說明 |
|------|------|
| `get_scene_state` | 讀取場景所有物件（Claude用來感知場景） |
| `create_geometry` | 創建 box / cylinder / sphere |
| `push_pull` | 面擠出操作（SketchUp核心功能） |
| `set_material` | 設定材質（wood/metal/glass/concrete...） |
| `move_object` | 移動物件 |
| `execute_batch` | 批次執行多操作 |
| `delete_object` | 刪除物件 |
| `clear_scene` | 清空場景 |

## 在 Claude 桌面版中使用

```
你：幫我建一個 5x4 公尺的客廳，牆高 2.8 米

Claude → get_scene_state()           # 感知當前場景
Claude → execute_batch([
  create_box(name="地板", 5000,100,4000),
  create_box(name="北牆", 5000,2800,200),
  create_box(name="南牆", 5000,2800,200),
  create_box(name="東牆", 200,2800,4000),
  create_box(name="西牆", 200,2800,4000),
])
Claude → set_material(id, "concrete") # 設定材質

✅ 客廳建模完成！
```

## Face Reference 格式

```
{obj_id}.face.{side}

side 可以是: top | bottom | front | back | left | right

範例: "abc123.face.top"
```

## 下一步開發

- [ ] 接入 wgpu 渲染視窗（即時 3D 顯示）
- [ ] SSE 模式（支援 Web 版 Claude）
- [ ] 更完整的 B-Rep（truck crate 整合）
- [ ] GLTF / OBJ 匯出
- [ ] 真實 Boolean 運算（Open CASCADE FFI）

## 目錄結構

```
src/
├── main.rs          # stdio MCP 主迴圈
├── mcp/
│   └── mod.rs       # JSON-RPC 處理器 + Tool 定義
└── scene/
    ├── mod.rs
    ├── geometry.rs  # B-Rep 幾何類型
    ├── operations.rs # CAD 指令協議
    └── state.rs     # 場景狀態管理器
```
