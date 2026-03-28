# SKP App Restore Progress 2026-03-28

## Scope

This note records the progress made on the bridge-based SKP import path and the work done to restore imported SKP content into the Kolibri/K3D app scene.

Primary goal of this round:

- Verify `SKP -> bridge JSON -> UnifiedIR -> Scene -> K3D runtime`
- Make large imported scenes load and serialize reliably
- Add enough diagnostics to prove whether imported content is actually present inside the app

## Verified Working

### 1. Bridge import data is valid for tested samples

Validated samples:

- `docs/sample/SKP_IMPORT_TEST.SKP`
- `docs/sample/component_sample.SKP`

Observed results:

- `SKP_IMPORT_TEST.SKP`
  - `3 meshes / 3 instances / 3 groups / 1 component_def`
- `component_sample.SKP`
  - `62 meshes / 709 instances / 187 groups / 11 component_defs`

Generated verification files:

- `docs/sample/SKP_IMPORT_TEST_bridge_export.json`
- `docs/sample/SKP_IMPORT_TEST_bridge_ir_verify.json`
- `docs/sample/SKP_IMPORT_TEST_bridge_scene_verify.json`
- `docs/sample/component_sample_bridge_export.json`
- `docs/sample/component_sample_bridge_ir_verify.json`
- `docs/sample/component_sample_bridge_scene_verify.json`

Conclusion:

- Bridge path preserves component definitions, repeated instances, and groups.

### 2. Large scene export and reload now work

The generated large app scene file now exists and is usable:

- `docs/sample/component_sample_scene.json`

This required fixing two core issues:

- Large scene save path no longer builds the entire JSON string in memory first
- `HeMesh` JSON serialization now avoids the numeric-key map problem that broke reload

### 3. K3D runtime can load the generated scene

Runtime verification file:

- `logs/startup_scene_state.json`

When launched with `component_sample_scene.json`, the state file reports:

- `objects = 709`
- `groups = 187`
- `component_defs = 11`

Meaning:

- The imported SKP-derived scene is not only valid as intermediate data
- It is successfully reconstructed into the app runtime scene model

## Code Changes Made In This Round

### `app/src/app.rs`

- Added startup scene loading support
- Added startup scene state JSON output for runtime verification
- Added startup screenshot plumbing and phase diagnostics
- Added import/audit logging improvements
- Changed SKP scene replacement behavior so imported scene does not stack on top of the default scene
- Cleared stale hidden/editing state when replacing the scene
- Added a light heartbeat repaint request to help deferred startup/test automation progress

### `app/src/main.rs`

- Added/used CLI support for opening a scene on startup
- Used startup env-based scene loading for runtime verification flow

### `crates/core/src/scene.rs`

- Changed scene saving to streamed JSON writing instead of full in-memory string assembly

### `crates/core/src/halfedge.rs`

- Implemented stable custom serde for `HeMesh`
- Avoided JSON numeric-key decode failures on reload

## Important Findings

### 1. Scene restoration is ahead of GUI verification

Current strongest confirmed statement:

- Imported SKP-derived scene data can be restored into the K3D runtime scene successfully.

What is still not fully proven yet:

- Pixel-level viewport rendering parity with the original SKP file in an automated way

### 2. Startup screenshot automation is not yet stable

State observed:

- `startup_screenshot_path` can now be passed into the app correctly
- `startup_screenshot_armed` appears in `logs/import_audit.log`
- But the later screenshot countdown/save phases do not reliably continue

Implication:

- The blocker is not scene reconstruction
- The blocker is the follow-up GUI automation/repaint path used for proof screenshots

### 3. Test bridge screenshot path is partially working but not trustworthy yet

Observed:

- `app/test_bridge` can produce a screenshot in some runs
- But in at least one successful screenshot run, it captured a scene with `9` default/test objects, not the intended `component_sample` scene

Implication:

- There were multiple app instances or mismatched runtime contexts during testing
- Screenshot success alone is not enough unless tied to the verified target scene instance

### 4. MCP bridge is usable but instance management is currently messy

Observed:

- K3D has a usable MCP HTTP bridge path
- MCP responses were successfully returned from `/health` and `/mcp`
- But some MCP responses came from the wrong live instance/scene

Example mismatch seen during testing:

- `startup_scene_state.json` reported `709 / 187 / 11`
- MCP `get_scene_state` at another point returned only a `TestBox` scene

Implication:

- There were conflicting or stale runtime instances/bridges
- Any further MCP-based validation should be done only after enforcing a single clean app instance

## Current Blockers

### Blocker A: single-instance control

Need a strict rule during debugging:

- only one `kolibri-cad.exe` may be running
- only one active MCP bridge on port `3001`

Without this, scene state, screenshots, and MCP responses can refer to different app instances.

### Blocker B: viewport proof automation

Need a reliable way to prove the actual K3D viewport matches the loaded scene:

- either stabilize startup screenshot path
- or stabilize MCP/test automation against one known live instance
- or add a dedicated screenshot/export tool callable from MCP

## Recommended Next Steps

1. Enforce single-instance launch discipline before every verification run
2. Use one owner for MCP during a given debugging session to avoid collisions
3. Add or expose a screenshot-capable MCP tool so the live GUI instance can be queried and captured directly
4. Re-run verification with:
   - one app instance
   - one MCP bridge
   - `component_sample_scene.json`
5. Compare the resulting K3D viewport capture with the original SketchUp sample visually

## Suggested Session Protocol

Before continuing:

- stop all `kolibri-cad.exe`
- verify port `3001` is free
- launch one app instance only
- choose one owner for MCP operations

Then validate in this order:

1. `startup_scene_state.json`
2. MCP `/health`
3. MCP `get_scene_state`
4. screenshot from the same live instance

## Files Worth Checking First

- `app/src/app.rs`
- `app/src/main.rs`
- `crates/core/src/scene.rs`
- `crates/core/src/halfedge.rs`
- `logs/startup_scene_state.json`
- `logs/import_audit.log`
- `docs/sample/component_sample_scene.json`

## Bottom Line

As of 2026-03-28:

- Bridge-based SKP restoration into the app runtime scene is working
- Large scene save/reload is working
- The remaining issue is reliable live-GUI verification, not core scene reconstruction
