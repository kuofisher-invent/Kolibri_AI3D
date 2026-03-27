# Kolibri Ai3D

A desktop 3D CAD modeling application built with **Rust + egui + wgpu**, combining SketchUp-style interactive modeling with Tekla-style structural steel tools and AI-powered MCP integration.

## Features

### 3D Modeling
- **SketchUp-style tools**: Line, Arc, Rectangle, Circle, Box, Cylinder, Sphere, Push/Pull, Offset, Follow Me
- **Transform**: Move (with gizmo), Rotate (3-step protractor), Scale (with handles)
- **Selection**: Object/Face/Edge modes, Crossing/Window rubber band, Ctrl+Click multi-select
- **Snap inference**: Endpoint, Midpoint, Origin, Axis, Parallel, Perpendicular, Tangent, On-Edge, On-Face
- **4-layer inference engine**: Geometry + Context + Semantic + Intent scoring
- **Operations**: Copy/Paste/Cut, Mirror, Align, Distribute, Polar/Linear Array, CSG Boolean

### Architecture Tools
- **Wall tool** (W): Click two points to create parametric walls (configurable thickness/height)
- **Slab tool**: Click two corners for floor slabs
- **Multi-floor**: Floor selector with automatic Y-offset
- **Steel mode**: Column, Beam, Brace, Plate, Connection tools

### Rendering
- **PBR**: Cook-Torrance BRDF (GGX + Schlick Fresnel + Smith geometry)
- **Render modes**: Shaded, Wireframe, X-Ray, Hidden Line, Monochrome, Sketch
- **Performance**: Frustum culling, LOD, shadow caching, rstar spatial index
- **Visual**: Dark mode, scale bar, viewport axes indicator, compass rose

### File I/O
| Format | Import | Export |
|--------|--------|--------|
| OBJ    | Real mesh + .mtl materials | Full geometry + .mtl + UV |
| STL    | Real mesh (vertex dedup) | Binary + ASCII, mm/m units |
| DXF    | LINE/3DFACE/CIRCLE | Full geometry + LAYER colors |
| glTF   | — | Per-object nodes + PBR materials |
| SKP    | SketchUp bridge (Ruby plugin) | — |
| K3D    | Native scene format | Native scene format |

### MCP Server (31 tools)
AI assistants (Claude Desktop, ChatGPT) can control the 3D scene via Model Context Protocol:

```bash
# Claude Desktop (stdio)
cargo run --bin kolibri-mcp-server

# ChatGPT (HTTP/SSE + Web Dashboard)
cargo run --bin kolibri-mcp-server -- --http
# Open http://localhost:3001/
```

Tools include: scene query, object CRUD, push/pull, rotate, scale, align, mirror, duplicate, material, undo/redo, import/export, create_wall, create_slab, create_room, create_column_grid, batch_create, measure, and more.

### UI
- **Command palette** (Ctrl+P): Search and execute any command
- **Scene outliner**: Tree view with groups, visibility toggle, inline rename
- **Camera bookmarks**: Save and restore viewport positions
- **Toast notifications**: Non-blocking status messages
- **Quick material palette**: Recent materials for fast switching

## Architecture

```
Kolibri_Ai3D/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── core/               # Pure logic (no GUI) — scene, halfedge, collision, CSG, geometry kernel
│   ├── io/                 # File I/O — DXF, OBJ, STL, glTF, CAD import, DWG parser
│   └── mcp/                # MCP Server — protocol, adapter, stdio/HTTP transports, dashboard
└── app/                    # GUI application — egui + wgpu renderer
```

- **4-crate workspace** for independent compilation and testing
- **Three-layer Store** (Pascal Editor style): SceneStore / ViewerStore / EditorStore
- **Command Pattern Undo/Redo**: Diff-based for common ops, full snapshot fallback
- **GeometryKernel trait**: Abstraction for future NURBS/B-Rep backend

## Build & Run

```bash
# Prerequisites: Rust 1.75+, GPU with Vulkan/Metal/DX12

# Build everything
cargo build

# Run the GUI app
cargo run -p kolibri-cad

# Run MCP server (stdio for Claude Desktop)
cargo run --bin kolibri-mcp-server

# Run MCP server (HTTP + Dashboard for ChatGPT)
cargo run --bin kolibri-mcp-server -- --http

# Run MCP test harness
cargo run --bin kolibri-mcp-test

# Test core crate (no GPU required)
cargo test -p kolibri-core
```

## Keyboard Shortcuts

| Key | Action | Key | Action |
|-----|--------|-----|--------|
| Space | Select | M | Move |
| Q | Rotate | S | Scale |
| L | Line | A | Arc |
| R | Rectangle | C | Circle |
| B | Box | P | Push/Pull |
| W | Wall | E | Eraser |
| T | Tape Measure | D | Dimension |
| G | Group | F | Offset |
| Z | Zoom Extents | O | Orbit |
| H | Pan | . | Zoom to Selection |
| Ctrl+C/V/X | Copy/Paste/Cut | Ctrl+D | Duplicate |
| Ctrl+M | Mirror X | Ctrl+I | Invert Selection |
| Ctrl+P | Command Palette | Ctrl+A | Select All |
| Ctrl+Shift+C/V | Copy/Paste Properties | Alt+H | Hide Selected |
| Alt+Shift+H | Show All | Alt+I | Isolate Selected |
| F1 | Help | F12 | Console |
| 1/2/3 | Front/Top/Iso View | 4/6/8 | Left/Right/Back View |

## License

Proprietary. All rights reserved.
