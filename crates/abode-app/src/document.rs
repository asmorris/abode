//! An open house file: the project, its storage, and undo/redo history.
//!
//! The UI edits `project` directly. Once an edit "settles" (no text field
//! focused, no drag in progress) we `commit`: diff against the last committed
//! state, append change-log entries, and push an undo snapshot. That turns a
//! burst of typing or a whole drag gesture into a single undo step.

use abode_core::diff::record_changes;
use abode_core::storage::Store;
use abode_core::{AttachmentId, ChangeEntry, Project};
use anyhow::Result;
use chrono::Utc;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const UNDO_LIMIT: usize = 200;

pub struct Document {
    pub project: Project,
    pub store: Store,
    pub path: Option<PathBuf>,
    committed: Project,
    undo: Vec<Project>,
    redo: Vec<Project>,
    dirty: bool,
}

impl Document {
    fn from_parts(project: Project, store: Store, path: Option<PathBuf>) -> Self {
        Self {
            committed: project.clone(),
            project,
            store,
            path,
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
        }
    }

    pub fn new_with(project: Project) -> Result<Self> {
        Ok(Self::from_parts(project, Store::create()?, None))
    }

    pub fn open(path: &Path) -> Result<Self> {
        let (store, project) = Store::open(path)?;
        Ok(Self::from_parts(project, store, Some(path.to_path_buf())))
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty || self.project != self.committed
    }

    pub fn display_name(&self) -> String {
        match &self.path {
            Some(p) => p
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into()),
            None if !self.project.info.name.trim().is_empty() => self.project.info.name.clone(),
            None => "Untitled".into(),
        }
    }

    /// Record pending edits as one undo step and log them. Returns true if
    /// anything changed.
    pub fn commit(&mut self, author: &str) -> bool {
        if self.project == self.committed {
            return false;
        }
        record_changes(&mut self.project, &self.committed, author, Utc::now());
        let previous = std::mem::replace(&mut self.committed, self.project.clone());
        self.undo.push(previous);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
        true
    }

    /// Lock/unlock a room's shape without creating an undo step or log entry
    /// (it's a UI safety catch, not a change to the house).
    pub fn set_room_locked(&mut self, room: abode_core::RoomId, locked: bool) {
        for p in [&mut self.project, &mut self.committed] {
            for l in &mut p.levels {
                if let Some(r) = l.rooms.get_mut(&room) {
                    r.shape_locked = locked;
                }
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || self.project != self.committed
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self, author: &str) {
        self.commit(author);
        if let Some(prev) = self.undo.pop() {
            let current = std::mem::replace(&mut self.project, prev);
            self.redo.push(current);
            self.committed = self.project.clone();
            self.dirty = true;
        }
    }

    pub fn redo(&mut self, author: &str) {
        self.commit(author);
        if let Some(next) = self.redo.pop() {
            let current = std::mem::replace(&mut self.project, next);
            self.undo.push(current);
            self.committed = self.project.clone();
            self.dirty = true;
        }
    }

    /// Attachments referenced anywhere, including undo/redo history, so
    /// undoing a delete never leaves a note pointing at a missing photo.
    fn live_attachments(&self) -> BTreeSet<AttachmentId> {
        let mut keep = self.project.referenced_attachments();
        for p in self.undo.iter().chain(self.redo.iter()) {
            keep.extend(p.referenced_attachments());
        }
        keep
    }

    pub fn save(&mut self, path: &Path, author: &str, message: &str) -> Result<()> {
        self.commit(author);
        let keep = self.live_attachments();
        self.store.save(&self.project, path, &keep, author, message)?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// Export as plain files into a new folder inside `parent`; returns it.
    pub fn export_documents(&mut self, parent: &Path, include_private: bool, author: &str) -> Result<std::path::PathBuf> {
        self.commit(author);
        self.store.export_documents(&self.project, parent, include_private, chrono::Local::now().date_naive())
    }

    pub fn export_shared(&mut self, path: &Path, author: &str) -> Result<()> {
        self.commit(author);
        self.store.export_shared(&self.project, path)
    }

    /// Replace the current state with an old revision, keeping the change
    /// log (and adding an entry saying what happened).
    pub fn restore(&mut self, mut snapshot: Project, label: &str, author: &str) {
        self.commit(author);
        snapshot.changelog = self.project.changelog.clone();
        snapshot
            .changelog
            .push(ChangeEntry::manual(author, format!("Restored saved version from {label}"), None));
        self.project = snapshot;
        self.commit(author);
    }
}
