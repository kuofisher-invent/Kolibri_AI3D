# SKP Import Crash Handoff

Last updated: 2026-03-27

## Context

This note summarizes the SKP import stability work completed up to the latest session, plus the most likely next fixes.

The main user-facing symptom was:

- importing a large `.skp` made the app appear frozen
- after recent changes, the app no longer only "looked frozen"
- instead, the process eventually consumed huge memory and exited

## What Was Changed

### 1. Import parsing moved off the UI thread

The direct synchronous import path in `app/src/tools.rs` was changed so:

- file selection no longer immediately runs `import_manager::import_file()` on the UI thread
- SKP/DWG/PDF smart import now starts a background import task
- the main thread polls for completion from `KolibriApp::update()`

Relevant files:

- `app/src/app.rs`
- `app/src/tools.rs`

### 2. Scene building moved off the UI thread

The confirm action in the import overlay no longer directly calls:

- `build_scene_from_ir(&mut self.scene, &ir_data)`

Instead it now starts a background scene build task and returns control to the UI.

Relevant files:

- `app/src/app.rs`
- `app/src/overlay.rs`

### 3. Background task infrastructure was added

`KolibriApp` now includes background-task state:

- `background_task_rx`
- `background_task_label`
- import/build result handling
- polling in `update()`

Relevant types added in `app/src/app.rs`:

- `BackgroundSceneBuild`
- `BackgroundTaskResult`

### 4. Rebuild and launch status

The project successfully passed:

- `cargo check -p kolibri-cad --bin kolibri-cad`
- `cargo build -p kolibri-cad --bin kolibri-cad`

The debug app was rebuilt and relaunched successfully after the changes.

## Current Observed Runtime Behavior

The latest monitored SKP import did not merely "hang forever".

Observed timeline from process monitoring:

- app remained responsive at first
- memory climbed rapidly during import/build
- memory then jumped from hundreds of MB into multiple GB
- process reached roughly `8.7 GB` private memory
- process then disappeared

Observed sample timeline:

- `20:10:26` about `809 MB`
- `20:10:29` about `1.7 GB`, `Responding=False`
- `20:10:37` about `8.7 GB`
- `20:10:43` process gone

This strongly suggests:

- the problem is no longer only "UI thread blocking"
- there is still a large-memory failure in the SKP post-import path

## Important User Observation

Right before the crash, the user reported seeing a popup like:

- `儲存完成`

That matters because it suggests the crash may happen after import/build work is mostly done, during a follow-up step such as:

- autosave
- scene version/save side effects
- post-import scene integration
- renderer / spatial index rebuild

## Most Likely Remaining Failure Points

Based on the current code and the latest monitoring result, the highest-probability suspects are:

1. `build_scene_from_ir()` still causing very large transient allocations for large SKP scenes
2. replacing `self.scene` with a fully built cloned scene on the main thread
3. post-import `zoom_extents()`
4. spatial index rebuild after scene replacement
5. autosave or another save-related path triggered by scene/version changes
6. renderer-side geometry upload / cache rebuild for the imported scene

## What Has NOT Been Solved Yet

- large SKP import is still not safe for production use
- there is not yet a visible "busy" overlay for background import/build
- phase-by-phase memory logging has not yet been added
- autosave has not yet been isolated from the SKP import crash path

## Recommended Next Steps

### Immediate debugging steps

1. Add phase logging around background scene build:
   - start import
   - bridge JSON loaded
   - IR ready
   - scene build start
   - scene build done
   - scene assignment start
   - zoom extents start/done
   - spatial index rebuild start/done
   - autosave start/done

2. Record memory at each phase.

3. Temporarily disable autosave during large import completion to test whether the crash is save-related.

4. Temporarily skip `zoom_extents()` after SKP import completion to see whether the crash moves or disappears.

5. Temporarily skip spatial index rebuild after SKP import completion to isolate index cost.

### Safe-mode mitigation

Add a temporary "SKP safe mode" path:

- parse SKP
- build minimal scene or partial scene
- delay full renderer/index work
- allow the user to explicitly continue loading heavy data

This would likely prevent immediate 8+ GB spikes while debugging.

## Suggested Implementation Order

1. Add detailed phase/memory logging first.
2. Disable autosave for post-import path and retest.
3. Disable `zoom_extents()` and retest.
4. Disable spatial index rebuild and retest.
5. If still crashing, instrument `build_scene_from_ir()` internals more deeply.

## Files Most Relevant For The Next Session

- `app/src/app.rs`
- `app/src/tools.rs`
- `app/src/overlay.rs`
- `app/src/import/import_manager.rs`
- `app/src/renderer.rs`
- `app/src/scene_hierarchy.rs`
- `app/src/panels.rs`
- `crates/core/src/scene.rs`

## Current Practical Conclusion

As of this handoff:

- UI-thread freezing for import/build was partially addressed
- the app now fails later and more clearly
- the remaining blocker is a large-memory post-import crash, not just synchronous UI work
