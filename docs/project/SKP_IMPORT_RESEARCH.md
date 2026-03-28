# SKP Import Research

Last updated: 2026-03-27

## 2026-03-27 Status Update

The repo is no longer at the "placeholder scene reconstruction" stage described below.
The current implementation has already moved several major pieces into production shape:

- SketchUp bridge export is now definition-based instead of flattening repeated geometry.
- Rust import now supports `SkpBackend`-style bridge loading into `UnifiedIR`.
- `ImportCache` exists as a staging layer for `meshes`, `instances`, `groups`, `component_defs`, and `materials`.
- `build_scene_from_ir()` / import manager now reconstruct real mesh objects, groups, component definitions, and component instances.
- `SceneObject` has a formal `component_def_id`, with legacy tag fallback only for older data.
- The app UI now has a scene hierarchy view plus `definition -> instances` browsing for imported components.
- The app also has an explicit component editing mode with sync/exit workflow.

In other words:

1. `bridge JSON -> UnifiedIR` is working
2. `UnifiedIR -> Scene` is working
3. app-side component/group editing UX is partially working

What is still incomplete is not the basic import path, but the next refinement layer:

- make the bridge preserve even more SketchUp metadata
- keep improving very large scene performance
- evolve `ComponentDef` from snapshot-like storage toward a cleaner definition graph
- eventually replace the bridge with an FFI/native backend if SDK access is ready

## Current Recommended Architecture

Current practical pipeline in this repo:

```text
SKP
-> SketchUp bridge backend
-> bridge JSON
-> UnifiedIR
-> ImportCache
-> Scene
-> Scene hierarchy / component editing UI
```

Current recommendation:

- treat the SketchUp bridge as the production backend
- keep Rust as the owner of parsing, validation, caching, and scene reconstruction
- keep the door open for a future native SDK/FFI backend without rewriting the app-side pipeline

Related status notes:

- `docs/project/SKP_STATUS_UPDATE_2026_03_27.md`
- `docs/project/COMPONENT_EDITING_STATUS.md`

## Goal

Use Rust to read SketchUp `.skp` files and import enough structure into Kolibri so the app can preserve:

- mesh geometry
- group hierarchy
- component definitions and repeated instances
- materials / textures
- layer or tag-like metadata
- names and transforms that are useful for editing in the app

This note is based on the current Kolibri codebase and the practical constraints of implementing an SKP pipeline in Rust.

## Current State In This Repo

The project already has the right target-side data model for SKP-like content:

- `app/src/import/unified_ir.rs`
  - `meshes`
  - `instances`
  - `groups`
  - `component_defs`
  - `materials`
- `crates/core/src/scene.rs`
  - `Scene.groups`
  - `Scene.component_defs`
  - per-object `tag`, `parent_id`, `material`, and mesh shape

However, the current SKP importer is only a placeholder:

- `app/src/import/skp_importer.rs`
  - tries ZIP inspection
  - performs heuristic binary scanning
  - falls back to a fake bounding box mesh
  - does not reconstruct real component/group relationships
  - does not map imported data into Kolibri component definitions

## Important Technical Finding

For this project, "read SKP in Rust" and "preserve SketchUp components correctly" should be treated as two different problems:

1. Low-level file decoding
2. Kolibri scene reconstruction

The repo is already fairly ready for problem 2.
The hard part is problem 1.

SKP is not a friendly interchange format in the same way OBJ or DXF are. Even if we can scrape vertices from bytes, that is not enough for Kolibri's real use case, because the app needs:

- shared component definitions
- repeated instances with transforms
- nested groups
- material assignment
- stable names

Without those, an SKP import is only "displayable geometry", not an editable SketchUp-like scene.

## Practical Conclusion

There are three implementation paths, ordered from most realistic to least realistic.

### Path A: Rust + native SDK bridge

Recommended for production-quality `.skp` import.

Architecture:

- Rust owns the import pipeline and Kolibri-side IR conversion.
- A thin FFI layer calls a native SKP-capable SDK/library.
- The native layer returns normalized scene data to Rust.
- Rust converts that into `UnifiedIR`, then into `Scene`.

Why this is the best path:

- preserves component definitions and instances
- preserves hierarchy better
- avoids reverse-engineering the full binary format
- lets Kolibri stay Rust-first at the application layer

Tradeoffs:

- build complexity
- platform-specific packaging
- licensing / redistribution review

### Path B: Converter pipeline

Recommended as the fastest usable path if full native SDK embedding is too heavy.

Architecture:

- import `.skp`
- invoke an external converter step
- convert to an easier intermediate format such as OBJ/glTF/DAE
- parse that in Rust
- rebuild as much metadata as the intermediate retains

Why this helps:

- much lower implementation risk
- geometry import becomes reliable quickly
- easier CI and debugging

Main drawback:

- most converters flatten or weaken component-instance semantics
- materials may survive, but repeated component definitions often degrade into duplicated meshes

### Path C: Pure Rust reverse-engineered SKP parser

Not recommended as the first serious implementation target.

Reason:

- highest engineering cost
- highest correctness risk
- weakest documentation situation
- difficult test coverage without a large real SKP corpus

This path only makes sense if Kolibri explicitly wants to invest in long-term native SKP decoding as a core differentiator.

## What Kolibri Actually Needs From SKP

Kolibri does not need every SketchUp concept on day one.
It needs the subset that maps cleanly into the current app model.

### Minimum importable entities

1. Component definition
2. Component instance
3. Group
4. Face mesh
5. Material
6. Layer/tag name
7. Transform matrix
8. Entity name

### Mapping to current Rust IR

SketchUp concept -> Kolibri target

- component definition -> `IrComponentDef`
- component instance -> `IrInstance { component_def_id, transform }`
- group -> `IrGroup`
- raw mesh/face set -> `IrMesh`
- material -> `IrMaterial`
- per-entity layer/tag -> `IrInstance.layer` or object `tag`

### Mapping to current Scene model

Recommended scene reconstruction rules:

- one unique SketchUp component definition -> one Kolibri `ComponentDef`
- each SketchUp instance -> one Kolibri object tagged as `元件:{def_id}`
- each nested group -> one Kolibri `GroupDef`
- preserve parent-child relationships using `parent_id`
- if only triangulated mesh is available, import as `Shape::Mesh`

## Suggested Rust-Side Import Pipeline

### Stage 1: Parse into a source-neutral SKP scene

Introduce a parser-side model that is richer than today's `UnifiedIR`.

Suggested structs:

```rust
pub struct SkpScene {
    pub nodes: Vec<SkpNode>,
    pub component_defs: Vec<SkpComponentDef>,
    pub meshes: Vec<SkpMesh>,
    pub materials: Vec<SkpMaterial>,
    pub stats: SkpStats,
}

pub struct SkpNode {
    pub id: String,
    pub name: String,
    pub kind: SkpNodeKind,
    pub parent_id: Option<String>,
    pub transform: [f32; 16],
    pub material_id: Option<String>,
    pub tag: Option<String>,
}

pub enum SkpNodeKind {
    Group,
    ComponentInstance { def_id: String },
    LooseMesh { mesh_id: String },
}
```

Why add this layer:

- keeps SKP-specific concerns out of UI code
- lets us support multiple backends later
- makes SDK-backed and converter-backed importers converge on one model

### Stage 2: Convert `SkpScene` to `UnifiedIR`

Conversion rules:

- `SkpMesh` -> `IrMesh`
- `SkpComponentDef` -> `IrComponentDef`
- `SkpNodeKind::ComponentInstance` -> `IrInstance`
- `SkpNodeKind::Group` -> `IrGroup`
- `SkpMaterial` -> `IrMaterial`

### Stage 3: Build Kolibri `Scene`

Current gap in the repo:

- `app/src/import/import_manager.rs` mostly builds bounding boxes from `ir.meshes`
- it does not yet materialize `ir.instances`, `ir.groups`, and `ir.component_defs` into the full app scene graph

That means even a good SKP parser would still lose value today unless `build_scene_from_ir()` is upgraded.

## Required Code Changes

### 1. Replace the current SKP importer contract

Current file:

- `app/src/import/skp_importer.rs`

Recommended split:

- `app/src/import/skp/mod.rs`
- `app/src/import/skp/source_scene.rs`
- `app/src/import/skp/unified.rs`
- `app/src/import/skp/backend_sdk.rs`
- `app/src/import/skp/backend_converter.rs`

### 2. Extend build pipeline to honor instances and groups

Current bottleneck:

- `app/src/import/import_manager.rs`

Needed behavior:

- instantiate reusable component definitions once
- create scene objects per instance transform
- reconstruct group hierarchy
- assign parent-child relationships
- preserve names and tags

### 3. Add mesh import path that does not collapse everything into boxes

Right now `build_scene_from_ir()` approximates imported meshes as boxes.
That is acceptable for DWG/PDF semantic import, but not for SketchUp.

SKP/OBJ/glTF imports should prefer:

- `Shape::Mesh(...)`

instead of:

- `add_box(...)`

### 4. Add material conversion

Target mapping suggestion:

- opaque RGB -> `MaterialKind::Paint(hex)`
- transparent -> `MaterialKind::Custom([r, g, b, a])`
- texture-bearing materials -> keep `texture_path`

## Proposed Delivery Plan

### Phase 0: Research completion

Deliverables:

- this note
- importer contract cleanup
- scene reconstruction design

### Phase 1: Make imported mesh scenes survive intact

Scope:

- upgrade `build_scene_from_ir()`
- support true mesh objects
- support `instances`, `groups`, `component_defs`

Outcome:

- OBJ can already benefit
- prepares the app for SKP once parsing improves

### Phase 2: Add SKP backend abstraction

Implement:

- `trait SkpBackend`

```rust
pub trait SkpBackend {
    fn load_scene(&self, path: &str) -> Result<SkpScene, String>;
}
```

Backends:

- `ConverterBackend`
- `SdkBackend`

Outcome:

- we can ship a practical fallback first
- we do not block the architecture on one parsing strategy

### Phase 3: Production SKP import

Preferred order:

1. SDK-backed import
2. converter fallback
3. optional native reverse-engineering later

## Risks

### Risk 1: Current importer assumptions are too optimistic

The current `skp_importer.rs` behaves like a heuristic extractor.
It should not be treated as proof that SKP structure is already being read.

### Risk 2: Geometry-only import is not enough

If we only recover triangles:

- components become duplicated meshes
- instance editing is lost
- group editing is lost
- scene organization becomes noisy

### Risk 3: Scene builder is currently the second bottleneck

Even after decoding SKP correctly, Kolibri still needs a better scene reconstruction stage.

## Recommendation

For Kolibri, the most sensible roadmap is:

1. first upgrade the Rust-side `UnifiedIR -> Scene` builder to preserve mesh instances, groups, and component defs
2. then introduce a pluggable SKP backend
3. use an SDK-backed reader for high-fidelity `.skp`
4. keep OBJ/glTF conversion as fallback

This gives the app real user value quickly while keeping the architecture compatible with a future full SKP solution.

## Immediate Next Task

The next implementation task should be:

"Refactor `build_scene_from_ir()` so imported mesh scenes can create real mesh objects, groups, and component definitions instead of only bounding boxes."

That work benefits:

- future SKP import
- current OBJ import
- any later glTF/IFC importer
