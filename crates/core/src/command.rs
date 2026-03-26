//! Command Pattern Undo/Redo — diff-based，取代全狀態 clone
//!
//! 常見操作（Move, Rotate, Scale, PushPull, Delete, Add）使用 Command，
//! 複雜操作退回到 snapshot() full clone。

use crate::scene::{SceneObject, Scene, MaterialKind, Shape};
use std::collections::HashMap;
use crate::halfedge::HeMesh;

/// Undo 堆疊中的一個條目：Command 或 full snapshot
#[derive(Clone, Debug)]
pub enum UndoEntry {
    /// Diff-based: 只記錄被改變的物件
    Diff(DiffSnapshot),
    /// Full clone（退回用，相容舊邏輯）
    Full(HashMap<String, SceneObject>, HeMesh),
}

/// 記錄哪些物件在操作前是什麼狀態
#[derive(Clone, Debug)]
pub struct DiffSnapshot {
    pub label: String,
    /// 操作前的物件快照（id → 操作前的物件狀態，None = 當時不存在）
    pub before: HashMap<String, Option<SceneObject>>,
}

impl Scene {
    /// Diff-based snapshot: 只備份指定 ID 的物件
    /// 用於已知哪些物件會被修改的操作（Move, Scale, Rotate, PushPull）
    pub fn snapshot_ids(&mut self, ids: &[&str], label: &str) {
        let mut before = HashMap::new();
        for &id in ids {
            before.insert(id.to_string(), self.objects.get(id).cloned());
        }
        self.undo_stack_v2.push(UndoEntry::Diff(DiffSnapshot {
            label: label.to_string(),
            before,
        }));
        if self.undo_stack_v2.len() > 50 {
            self.undo_stack_v2.remove(0);
        }
        self.redo_stack_v2.clear();
    }

    /// Diff-based snapshot for add operations: 記錄「之前不存在」
    pub fn snapshot_before_add(&mut self, id: &str, label: &str) {
        let mut before = HashMap::new();
        before.insert(id.to_string(), None); // 之前不存在
        self.undo_stack_v2.push(UndoEntry::Diff(DiffSnapshot {
            label: label.to_string(),
            before,
        }));
        if self.undo_stack_v2.len() > 50 {
            self.undo_stack_v2.remove(0);
        }
        self.redo_stack_v2.clear();
    }

    /// Full snapshot（相容舊邏輯，逐步替換）
    pub fn snapshot_full(&mut self) {
        self.undo_stack_v2.push(UndoEntry::Full(
            self.objects.clone(),
            self.free_mesh.clone(),
        ));
        if self.undo_stack_v2.len() > 50 {
            self.undo_stack_v2.remove(0);
        }
        self.redo_stack_v2.clear();
    }

    /// Undo（v2：支援 Diff + Full 混合）
    pub fn undo_v2(&mut self) -> bool {
        if let Some(entry) = self.undo_stack_v2.pop() {
            match entry {
                UndoEntry::Diff(diff) => {
                    // 記錄當前狀態到 redo
                    let mut redo_before = HashMap::new();
                    for (id, _) in &diff.before {
                        redo_before.insert(id.clone(), self.objects.get(id).cloned());
                    }
                    self.redo_stack_v2.push(UndoEntry::Diff(DiffSnapshot {
                        label: diff.label.clone(),
                        before: redo_before,
                    }));
                    // 還原
                    for (id, prev) in diff.before {
                        match prev {
                            Some(obj) => { self.objects.insert(id, obj); }
                            None => { self.objects.remove(&id); }
                        }
                    }
                }
                UndoEntry::Full(prev_objs, prev_mesh) => {
                    self.redo_stack_v2.push(UndoEntry::Full(
                        self.objects.clone(),
                        self.free_mesh.clone(),
                    ));
                    self.objects = prev_objs;
                    self.free_mesh = prev_mesh;
                }
            }
            self.version += 1;
            true
        } else {
            false
        }
    }

    /// Redo（v2）
    pub fn redo_v2(&mut self) -> bool {
        if let Some(entry) = self.redo_stack_v2.pop() {
            match entry {
                UndoEntry::Diff(diff) => {
                    let mut undo_before = HashMap::new();
                    for (id, _) in &diff.before {
                        undo_before.insert(id.clone(), self.objects.get(id).cloned());
                    }
                    self.undo_stack_v2.push(UndoEntry::Diff(DiffSnapshot {
                        label: diff.label.clone(),
                        before: undo_before,
                    }));
                    for (id, next) in diff.before {
                        match next {
                            Some(obj) => { self.objects.insert(id, obj); }
                            None => { self.objects.remove(&id); }
                        }
                    }
                }
                UndoEntry::Full(next_objs, next_mesh) => {
                    self.undo_stack_v2.push(UndoEntry::Full(
                        self.objects.clone(),
                        self.free_mesh.clone(),
                    ));
                    self.objects = next_objs;
                    self.free_mesh = next_mesh;
                }
            }
            self.version += 1;
            true
        } else {
            false
        }
    }
}
