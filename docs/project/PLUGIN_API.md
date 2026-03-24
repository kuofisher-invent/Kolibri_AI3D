# Kolibri_Ai3D Plugin HTTP Interface Specification

Version: 1.0.0
Date: 2026-03-23

## Overview

This document specifies the HTTP API that external AI plugins can use to
interact with the Kolibri_Ai3D CAD application. The app exposes a local
HTTP server on `http://localhost:9901` when plugin mode is enabled.

All requests and responses use JSON. The API follows JSON-RPC 2.0 over HTTP POST.

## Authentication

Plugins must include an `X-Plugin-Token` header with a token generated
by the app at startup. The token is displayed in the GUI and can be
copied from **Settings > Plugin Token**.

```
X-Plugin-Token: <token>
```

## Endpoint

```
POST http://localhost:9901/rpc
Content-Type: application/json
```

## Actor Identification

Every mutating call must include an `actor` object so the AI Audit Log
can attribute the change:

```json
{
  "actor": {
    "name": "Claude",
    "model": "claude-sonnet-4-20250514",
    "session_id": "a1b2c3d4"
  }
}
```

---

## Tools / Methods

### get_scene_state

Returns all objects in the current scene.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "get_scene_state",
    "arguments": {}
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [{
      "type": "text",
      "text": "{\"objects\": [...], \"count\": 5}"
    }]
  }
}
```

### create_box

Create a box primitive. Units are millimeters.

| Parameter  | Type     | Required | Default    |
|-----------|----------|----------|------------|
| name      | string   | no       | "Box"      |
| position  | [x,y,z]  | no       | [0,0,0]    |
| width     | number   | yes      |            |
| height    | number   | yes      |            |
| depth     | number   | yes      |            |
| material  | string   | no       | "concrete" |

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "create_box",
    "arguments": {
      "name": "Wall_1",
      "position": [0, 0, 0],
      "width": 3000,
      "height": 2700,
      "depth": 200,
      "material": "concrete"
    },
    "actor": { "name": "Claude", "model": "claude-sonnet-4-20250514", "session_id": "abc123" }
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [{"type": "text", "text": "{\"id\": \"uuid-here\"}"}]
  }
}
```

### create_cylinder

| Parameter  | Type     | Required | Default    |
|-----------|----------|----------|------------|
| name      | string   | no       | "Cylinder" |
| position  | [x,y,z]  | no       | [0,0,0]    |
| radius    | number   | yes      |            |
| height    | number   | yes      |            |
| material  | string   | no       | "concrete" |

### create_sphere

| Parameter  | Type     | Required | Default    |
|-----------|----------|----------|------------|
| name      | string   | no       | "Sphere"   |
| position  | [x,y,z]  | no       | [0,0,0]    |
| radius    | number   | yes      |            |
| material  | string   | no       | "concrete" |

### delete_object

| Parameter | Type   | Required |
|----------|--------|----------|
| id       | string | yes      |

### move_object

| Parameter | Type    | Required |
|----------|---------|----------|
| id       | string  | yes      |
| position | [x,y,z] | yes      |

### set_material

| Parameter | Type   | Required |
|----------|--------|----------|
| id       | string | yes      |
| material | string | yes      |

Available materials:
`concrete`, `wood`, `glass`, `metal`, `brick`, `white`, `black`,
`stone`, `marble`, `steel`, `aluminum`, `copper`, `gold`, `grass`,
`tile`, `plaster`

### clear_scene

No parameters. Removes all objects.

---

## MCP (Model Context Protocol) Integration

The same tool definitions are available via MCP stdio transport.
Run the app with `--mcp` flag for a headless MCP server:

```bash
kolibri-cad.exe --mcp
```

### Claude Desktop Configuration

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "kolibri-cad": {
      "command": "D:/AI_Design/Kolibri_Ai3D/app/target/release/kolibri-cad.exe",
      "args": ["--mcp"]
    }
  }
}
```

---

## AI Audit Log Format

All mutations are recorded in the AI Audit Log. Each entry contains:

```json
{
  "timestamp": "14:30:22",
  "actor": {
    "name": "Claude",
    "model": "claude-sonnet-4-20250514",
    "session_id": "a1b2c3d4"
  },
  "action": "create_box",
  "details": "3000x2700x200",
  "objects_affected": ["uuid-of-object"]
}
```

The log can be exported from the GUI (right panel > Records tab > Export)
or saved programmatically to `ai_log.json`.

---

## Error Codes

| Code   | Meaning              |
|--------|---------------------|
| -32700 | Parse error          |
| -32601 | Method not found     |
| -32602 | Invalid params       |
| -32603 | Internal error       |
| -32001 | Object not found     |
| -32002 | Authentication error |

## Rate Limits

No rate limits are enforced for localhost connections. Plugins should
be reasonable and avoid sending more than 100 requests per second.
