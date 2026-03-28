# Component Editing Status

Last updated: 2026-03-27

## Current UX

Kolibri now has an explicit component editing workflow for imported component instances.

Current app-side behavior:

- hierarchy shows `component definition -> instances`
- hierarchy also keeps the normal scene tree for groups / objects
- entering component editing sets `editing_component_def_id`
- viewport picking is restricted to the active component definition
- non-target objects are dimmed in the viewport
- overlay shows the current component editing banner

## Available Actions

In hierarchy:

- edit component
- finish sync
- exit editing
- focus
- select all
- show
- hide
- primary instance selection

In properties panel:

- edit component
- finish sync
- exit editing
- focus
- select all
- show
- hide
- primary instance selection

## Data Model Notes

- `SceneObject.component_def_id` is now the primary link for component instances
- legacy `tag = "元件:<id>"` behavior remains as fallback for older scene data
- `Scene` helper methods provide:
  - `component_instance_ids()`
  - `component_instance_count()`
  - `component_visible_instance_count()`
  - `set_component_instances_visible()`

## Remaining Gaps

- editing is still instance-driven rather than a fully isolated definition graph editor
- very large imported scenes still need more performance work
- component definition summaries can still be expanded
- full native SketchUp backend replacement is still future work
