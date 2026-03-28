# Encoding Marking Guide

This note is for files with mixed historical text encoding artifacts, especially:

- `app/src/panels.rs`
- `crates/core/src/scene.rs`

The goal is to separate:

- historical mojibake that should be preserved temporarily
- text that is clearly corrupted and should be fixed later
- newly added safe UTF-8 or ASCII blocks

## Why We Need This

Some legacy Rust source files contain text that looks like mojibake because they were edited across different encodings in the past.

These files may still compile, but direct rewrites can easily break:

- Rust string literals
- comments
- UI labels
- file encoding consistency

Because of that, we should mark blocks before large cleanup work.

## Recommended Markers

Use these comment markers consistently.

### 1. Historical Encoding Residue

Use when the block is ugly or unreadable, but is currently stable and should not be casually rewritten.

```rust
// ENCODING-LEGACY: historical mojibake kept as-is.
// Do not rewrite this block unless the file encoding is normalized first.
```

For larger blocks:

```rust
// ENCODING-LEGACY-BEGIN
// ...
// ENCODING-LEGACY-END
```

Meaning:

- content is not trusted as readable Chinese
- block may still be functionally valid
- avoid mass editing inside this region

### 2. Known Corrupted Text To Fix Later

Use when text is confirmed broken and should be normalized in a later cleanup pass.

```rust
// ENCODING-TODO: mojibake text should be normalized to UTF-8 Chinese.
// Verify original meaning before replacing strings or comments.
```

Meaning:

- this is not just legacy residue
- cleanup is expected
- replacement should be reviewed carefully

### 3. Verified Safe Block

Use for new or normalized code blocks that are safe to edit.

```rust
// ENCODING-OK: verified UTF-8 or ASCII block.
```

Meaning:

- block is safe for normal edits
- no special encoding handling needed

## File-Level Warning Header

At the top of risky files, add a short warning like this:

```rust
// ENCODING-WARNING:
// This file contains mixed historical text encoding artifacts.
// Prefer minimal edits.
// Normalize encoding before rewriting Chinese strings/comments broadly.
```

Recommended targets:

- `app/src/panels.rs`
- `crates/core/src/scene.rs`

## Practical Rule Set

When editing these files:

1. If a block currently compiles but contains mojibake, mark it `ENCODING-LEGACY`.
2. If a block is clearly broken and should be repaired later, mark it `ENCODING-TODO`.
3. Put all new helper logic in fresh UTF-8 files where possible.
4. Mark new code areas `ENCODING-OK`.
5. Prefer ASCII comments for safety if the file encoding is still uncertain.

## Suggested Rollout

### Phase 1

Only add markers. Do not change behavior.

### Phase 2

Move new logic into new UTF-8 files instead of expanding risky legacy files.

Example:

- move hierarchy rendering into a new helper module
- keep legacy UI strings in place temporarily

### Phase 3

Normalize legacy file encoding file-by-file, then replace `ENCODING-LEGACY` regions with readable UTF-8 text.

## Example Usage

```rust
// ENCODING-WARNING:
// This file contains mixed historical text encoding artifacts.
// Prefer minimal edits.

// ENCODING-LEGACY-BEGIN
// old mojibake comments or UI labels
// ENCODING-LEGACY-END

// ENCODING-OK: verified UTF-8 or ASCII block.
fn render_scene_hierarchy() {
    // safe new logic here
}

// ENCODING-TODO: normalize these labels after file encoding cleanup.
```

## Decision Summary

For current collaboration, use:

- `ENCODING-LEGACY` for stable historical residue
- `ENCODING-TODO` for known bad text to be repaired later
- `ENCODING-OK` for new safe code
- `ENCODING-WARNING` at file top for risky legacy files

This gives us a shared vocabulary before deeper cleanup with Claude.
