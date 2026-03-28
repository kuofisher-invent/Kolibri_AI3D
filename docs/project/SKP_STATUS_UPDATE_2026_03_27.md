# SKP Status Update

Last updated: 2026-03-27

## Current State

Kolibri now has a usable bridge-based SketchUp import pipeline.

Current pipeline:

```text
SKP
-> SketchUp bridge backend
-> bridge JSON
-> UnifiedIR
-> ImportCache
-> Scene
-> hierarchy / component editing UI
```

This is no longer a placeholder-only path.

## What Is Working

- SketchUp bridge export is definition-based instead of mostly flattened repeated meshes.
- Rust can load bridge JSON into `UnifiedIR`.
- `ImportCache` stores imported `meshes`, `instances`, `groups`, `component_defs`, and `materials`.
- Scene reconstruction creates real mesh objects, groups, component definitions, and component instances.
- `SceneObject` has formal `component_def_id` support, with legacy tag fallback for older data.
- The app provides:
  - scene hierarchy
  - component definition -> instances view
  - component editing mode
  - sync / exit / focus / select all / show / hide actions

## Verified Samples

The sample BSGS SketchUp files were used to verify the new bridge/import path.

Key confirmed result:

- geometry reuse is now preserved much better than the original flattened bridge export
- repeated component usage is visible in the app-side component hierarchy
- imported groups and component definitions now survive into `Scene`

## What Is Still Incomplete

- very large scene performance still needs work
- bridge metadata fidelity can still improve
- `ComponentDef` is still closer to a practical app snapshot than a fully normalized definition graph
- a native/FFI backend has not replaced the bridge yet

## Recommended Next Focus

1. Improve large-scene build and renderer performance.
2. Preserve more SketchUp metadata through the bridge.
3. Refine `ComponentDef` into a cleaner long-term graph model.
4. Keep the Rust-side pipeline stable so an FFI backend can replace the bridge later.
