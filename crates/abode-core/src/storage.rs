//! The `.abode` file: a single SQLite database.
//!
//! Layout:
//! - `meta`: format name/version, project id, timestamps
//! - one table per entity type (`rooms`, `notes`, ...), one row per entity:
//!   `(id, parent, position, data JSON)`. `parent` is the level id for
//!   geometry, or the target key (`room:<id>`) for notes etc.
//! - `attachments`: embedded files (metadata JSON + BLOB)
//! - `revisions`: full snapshots taken on each save (restore points)
//!
//! While a file is open we work on a private copy in a temp directory, so
//! adding a 50 MB manual doesn't touch the real file until you save, and a
//! crash can't corrupt it. Saving writes the working copy out atomically.

use crate::ids::*;
use crate::model::*;
use anyhow::{Context, Result, anyhow, bail};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub const FILE_EXTENSION: &str = "abode";
const FORMAT_NAME: &str = "abode";
pub const FORMAT_VERSION: i64 = 1;
/// Oldest restore points beyond this are pruned (the change log is kept forever).
const MAX_REVISIONS: i64 = 200;

const TABLES: &[&str] = &[
    "project",
    "levels",
    "nodes",
    "walls",
    "rooms",
    "openings",
    "objects",
    "roofs",
    "features",
    "buildings",
    "notes",
    "service_records",
    "reminders",
    "paints",
    "changelog",
];

fn schema() -> String {
    let mut s = String::from("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);\n");
    for t in TABLES {
        s += &format!(
            "CREATE TABLE IF NOT EXISTS {t} (id TEXT PRIMARY KEY, parent TEXT NOT NULL, position INTEGER NOT NULL, data TEXT NOT NULL);\n"
        );
    }
    s += "CREATE TABLE IF NOT EXISTS attachments (id TEXT PRIMARY KEY, data TEXT NOT NULL, content BLOB NOT NULL);\n";
    s += "CREATE TABLE IF NOT EXISTS revisions (id TEXT PRIMARY KEY, created TEXT NOT NULL, author TEXT NOT NULL, message TEXT NOT NULL, snapshot TEXT NOT NULL);\n";
    s
}

/// What the start screen shows about a house file, read without opening it.
#[derive(Clone, Debug)]
pub struct FileSummary {
    pub name: String,
    pub address: String,
    pub saved_at: Option<DateTime<Utc>>,
}

/// Read a house file's name and address (read-only, doesn't copy the file).
pub fn peek(path: &Path) -> Result<FileSummary> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    check_format(&conn)?;
    let data: String = conn.query_row("SELECT data FROM project LIMIT 1", [], |r| r.get(0))?;
    let v: Value = serde_json::from_str(&data)?;
    let text = |k: &str| v.get("info").and_then(|i| i.get(k)).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let saved_at = conn
        .query_row("SELECT value FROM meta WHERE key = 'saved_at'", [], |r| r.get::<_, String>(0))
        .optional()?
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc));
    Ok(FileSummary { name: text("name"), address: text("address"), saved_at })
}

#[derive(Clone, Debug)]
pub struct RevisionInfo {
    pub id: RevisionId,
    pub created: DateTime<Utc>,
    pub author: String,
    pub message: String,
}

pub struct Store {
    conn: Connection,
    attachments: HashMap<AttachmentId, AttachmentMeta>,
    _dir: TempDir,
}

fn now_str() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn working_dir() -> Result<(TempDir, PathBuf)> {
    let dir = tempfile::Builder::new().prefix("abode-").tempdir()?;
    let path = dir.path().join("working.abode");
    Ok((dir, path))
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(&schema())?;
    set_meta(conn, "format", FORMAT_NAME)?;
    set_meta(conn, "format_version", &FORMAT_VERSION.to_string())?;
    Ok(())
}

fn check_format(conn: &Connection) -> Result<()> {
    let format: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'format'", [], |r| r.get(0))
        .optional()
        .map_err(|_| anyhow!("this isn't an Abode house file"))?;
    if format.as_deref() != Some(FORMAT_NAME) {
        bail!("this isn't an Abode house file");
    }
    let version: i64 = conn
        .query_row("SELECT value FROM meta WHERE key = 'format_version'", [], |r| r.get::<_, String>(0))
        .optional()?
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    if version > FORMAT_VERSION {
        bail!("this file was made by a newer version of Abode; please update the app");
    }
    // Future: run migrations for version < FORMAT_VERSION here.
    Ok(())
}

fn strip(v: &mut Value, keys: &[&str]) {
    if let Value::Object(m) = v {
        for k in keys {
            m.remove(*k);
        }
    }
}

fn insert_row<T: Serialize>(conn: &Connection, table: &str, id: &str, parent: &str, pos: usize, value: &T) -> Result<()> {
    let data = serde_json::to_string(value)?;
    conn.prepare_cached(&format!("INSERT INTO {table}(id, parent, position, data) VALUES (?1, ?2, ?3, ?4)"))?
        .execute(params![id, parent, pos as i64, data])?;
    Ok(())
}

fn read_rows<T: DeserializeOwned>(conn: &Connection, table: &str) -> Result<Vec<(String, T)>> {
    let mut stmt = conn.prepare(&format!("SELECT parent, data FROM {table} ORDER BY position, id"))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (parent, data) = row?;
        let value = serde_json::from_str(&data).with_context(|| format!("damaged record in `{table}`"))?;
        out.push((parent, value));
    }
    Ok(out)
}

fn write_project(conn: &Connection, p: &Project) -> Result<()> {
    for t in TABLES {
        conn.execute(&format!("DELETE FROM {t}"), [])?;
    }
    let mut head = serde_json::to_value(p)?;
    strip(&mut head, &["levels", "buildings", "notes", "service_records", "reminders", "paints", "changelog"]);
    insert_row(conn, "project", &p.id.to_string(), "", 0, &head)?;

    for (i, level) in p.levels.iter().enumerate() {
        let lid = level.id.to_string();
        let mut v = serde_json::to_value(level)?;
        strip(&mut v, &["nodes", "walls", "rooms", "openings", "objects", "roofs", "features"]);
        insert_row(conn, "levels", &lid, "", i, &v)?;
        for (j, x) in level.nodes.values().enumerate() {
            insert_row(conn, "nodes", &x.id.to_string(), &lid, j, x)?;
        }
        for (j, x) in level.walls.values().enumerate() {
            insert_row(conn, "walls", &x.id.to_string(), &lid, j, x)?;
        }
        for (j, x) in level.rooms.values().enumerate() {
            insert_row(conn, "rooms", &x.id.to_string(), &lid, j, x)?;
        }
        for (j, x) in level.openings.values().enumerate() {
            insert_row(conn, "openings", &x.id.to_string(), &lid, j, x)?;
        }
        for (j, x) in level.objects.values().enumerate() {
            insert_row(conn, "objects", &x.id.to_string(), &lid, j, x)?;
        }
        for (j, x) in level.roofs.values().enumerate() {
            insert_row(conn, "roofs", &x.id.to_string(), &lid, j, x)?;
        }
        for (j, x) in level.features.values().enumerate() {
            insert_row(conn, "features", &x.id.to_string(), &lid, j, x)?;
        }
    }
    for (i, x) in p.buildings.iter().enumerate() {
        insert_row(conn, "buildings", &x.id.to_string(), "", i, x)?;
    }
    for (i, x) in p.notes.iter().enumerate() {
        insert_row(conn, "notes", &x.id.to_string(), &x.target.key(), i, x)?;
    }
    for (i, x) in p.service_records.iter().enumerate() {
        insert_row(conn, "service_records", &x.id.to_string(), &x.target.key(), i, x)?;
    }
    for (i, x) in p.reminders.iter().enumerate() {
        insert_row(conn, "reminders", &x.id.to_string(), &x.target.key(), i, x)?;
    }
    for (i, x) in p.paints.iter().enumerate() {
        let parent = x.target.entity().key();
        insert_row(conn, "paints", &x.id.to_string(), &parent, i, x)?;
    }
    for (i, x) in p.changelog.iter().enumerate() {
        insert_row(conn, "changelog", &x.id.to_string(), "", i, x)?;
    }
    Ok(())
}

fn load_project(conn: &Connection) -> Result<Project> {
    let (_, mut project): (String, Project) = read_rows(conn, "project")?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("the file has no house in it"))?;
    project.levels = read_rows::<Level>(conn, "levels")?.into_iter().map(|(_, l)| l).collect();
    let index: HashMap<String, usize> = project
        .levels
        .iter()
        .enumerate()
        .map(|(i, l)| (l.id.to_string(), i))
        .collect();
    let levels = &mut project.levels;
    for (parent, x) in read_rows::<Node>(conn, "nodes")? {
        if let Some(&i) = index.get(&parent) {
            levels[i].nodes.insert(x.id, x);
        }
    }
    for (parent, x) in read_rows::<Wall>(conn, "walls")? {
        if let Some(&i) = index.get(&parent) {
            levels[i].walls.insert(x.id, x);
        }
    }
    for (parent, x) in read_rows::<Room>(conn, "rooms")? {
        if let Some(&i) = index.get(&parent) {
            levels[i].rooms.insert(x.id, x);
        }
    }
    for (parent, x) in read_rows::<Opening>(conn, "openings")? {
        if let Some(&i) = index.get(&parent) {
            levels[i].openings.insert(x.id, x);
        }
    }
    for (parent, x) in read_rows::<PlacedObject>(conn, "objects")? {
        if let Some(&i) = index.get(&parent) {
            levels[i].objects.insert(x.id, x);
        }
    }
    for (parent, x) in read_rows::<RoofSection>(conn, "roofs")? {
        if let Some(&i) = index.get(&parent) {
            levels[i].roofs.insert(x.id, x);
        }
    }
    for (parent, x) in read_rows::<Feature>(conn, "features")? {
        if let Some(&i) = index.get(&parent) {
            levels[i].features.insert(x.id, x);
        }
    }
    project.buildings = read_rows(conn, "buildings")?.into_iter().map(|(_, x)| x).collect();
    project.notes = read_rows(conn, "notes")?.into_iter().map(|(_, x)| x).collect();
    project.service_records = read_rows(conn, "service_records")?.into_iter().map(|(_, x)| x).collect();
    project.reminders = read_rows(conn, "reminders")?.into_iter().map(|(_, x)| x).collect();
    project.paints = read_rows(conn, "paints")?.into_iter().map(|(_, x)| x).collect();
    project.changelog = read_rows(conn, "changelog")?.into_iter().map(|(_, x)| x).collect();
    project.ensure_site();
    Ok(project)
}

fn load_attachment_meta(conn: &Connection) -> Result<HashMap<AttachmentId, AttachmentMeta>> {
    let mut stmt = conn.prepare("SELECT data FROM attachments")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = HashMap::new();
    for row in rows {
        let meta: AttachmentMeta = serde_json::from_str(&row?)?;
        out.insert(meta.id, meta);
    }
    Ok(out)
}

fn gc_attachments(conn: &Connection, keep: &BTreeSet<AttachmentId>) -> Result<()> {
    let ids: Vec<String> = conn
        .prepare("SELECT id FROM attachments")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    for id in ids {
        let alive = AttachmentId::parse(&id).is_some_and(|a| keep.contains(&a));
        if !alive {
            conn.execute("DELETE FROM attachments WHERE id = ?1", params![id])?;
        }
    }
    Ok(())
}

/// Write a compacted copy of `conn` to `dest`, atomically (temp file + rename).
fn vacuum_into(conn: &Connection, dest: &Path) -> Result<()> {
    let name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "house.abode".into());
    let tmp = dest.with_file_name(format!(".{name}.saving"));
    let _ = fs::remove_file(&tmp);
    conn.execute("VACUUM INTO ?1", params![tmp.to_string_lossy()])
        .with_context(|| format!("couldn't write {}", dest.display()))?;
    fs::rename(&tmp, dest).with_context(|| format!("couldn't write {}", dest.display()))?;
    Ok(())
}

pub fn guess_mime(filename: &str) -> &'static str {
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "heic" => "image/heic",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "csv" => "text/csv",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
}

impl Store {
    /// A new, empty working store (for a house that hasn't been saved yet).
    pub fn create() -> Result<Self> {
        let (dir, path) = working_dir()?;
        let conn = Connection::open(&path)?;
        init(&conn)?;
        Ok(Self { conn, attachments: HashMap::new(), _dir: dir })
    }

    /// Open a house file (working on a private copy).
    pub fn open(path: &Path) -> Result<(Self, Project)> {
        let (dir, working) = working_dir()?;
        fs::copy(path, &working).with_context(|| format!("couldn't read {}", path.display()))?;
        let conn = Connection::open(&working)?;
        check_format(&conn)?;
        init(&conn)?;
        let project = load_project(&conn)?;
        let attachments = load_attachment_meta(&conn)?;
        Ok((Self { conn, attachments, _dir: dir }, project))
    }

    /// Save to `dest`. `keep` is every attachment still referenced (including
    /// by undo history); anything else is dropped from the file.
    pub fn save(
        &mut self,
        project: &Project,
        dest: &Path,
        keep: &BTreeSet<AttachmentId>,
        author: &str,
        message: &str,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        write_project(&tx, project)?;
        set_meta(&tx, "project_id", &project.id.to_string())?;
        set_meta(&tx, "saved_at", &now_str())?;
        set_meta(&tx, "app_version", env!("CARGO_PKG_VERSION"))?;
        add_revision(&tx, project, author, message)?;
        gc_attachments(&tx, keep)?;
        tx.commit()?;
        self.attachments.retain(|id, _| keep.contains(id));
        vacuum_into(&self.conn, dest)
    }

    /// Write a copy for a new owner: private content and save history removed.
    pub fn export_shared(&self, project: &Project, dest: &Path) -> Result<()> {
        let shared = project.sanitized_for_sharing();
        let keep = shared.referenced_attachments();
        let dir = tempfile::Builder::new().prefix("abode-export-").tempdir()?;
        let tmp = dir.path().join("export.abode");
        self.conn.execute("VACUUM INTO ?1", params![tmp.to_string_lossy()])?;
        let mut conn = Connection::open(&tmp)?;
        conn.pragma_update(None, "secure_delete", "ON")?;
        {
            let tx = conn.transaction()?;
            write_project(&tx, &shared)?;
            tx.execute("DELETE FROM revisions", [])?;
            gc_attachments(&tx, &keep)?;
            set_meta(&tx, "project_id", &shared.id.to_string())?;
            set_meta(&tx, "exported_at", &now_str())?;
            tx.commit()?;
        }
        // Rebuild the file so deleted private data can't be recovered from free pages.
        conn.execute_batch("VACUUM")?;
        vacuum_into(&conn, dest)
    }

    pub fn add_attachment(&mut self, filename: &str, bytes: &[u8]) -> Result<AttachmentMeta> {
        let meta = AttachmentMeta {
            id: AttachmentId::new(),
            filename: filename.to_string(),
            mime: guess_mime(filename).to_string(),
            size: bytes.len() as u64,
            added: Utc::now(),
        };
        self.conn.execute(
            "INSERT INTO attachments(id, data, content) VALUES (?1, ?2, ?3)",
            params![meta.id.to_string(), serde_json::to_string(&meta)?, bytes],
        )?;
        self.attachments.insert(meta.id, meta.clone());
        Ok(meta)
    }

    pub fn attachment_meta(&self, id: AttachmentId) -> Option<&AttachmentMeta> {
        self.attachments.get(&id)
    }

    pub fn read_attachment(&self, id: AttachmentId) -> Result<Vec<u8>> {
        self.conn
            .query_row(
                "SELECT content FROM attachments WHERE id = ?1",
                params![id.to_string()],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .ok_or_else(|| anyhow!("attachment is missing from the file"))
    }

    pub fn revisions(&self) -> Result<Vec<RevisionInfo>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, created, author, message FROM revisions ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, created, author, message) = row?;
            let (Some(id), Ok(created)) = (RevisionId::parse(&id), DateTime::parse_from_rfc3339(&created)) else {
                continue;
            };
            out.push(RevisionInfo { id, created: created.with_timezone(&Utc), author, message });
        }
        Ok(out)
    }

    pub fn load_revision(&self, id: RevisionId) -> Result<Project> {
        let snap: String = self.conn.query_row(
            "SELECT snapshot FROM revisions WHERE id = ?1",
            params![id.to_string()],
            |r| r.get(0),
        )?;
        Ok(serde_json::from_str(&snap)?)
    }
}

/// Store a snapshot (minus the change log, which only ever grows) unless
/// nothing changed since the last one.
fn add_revision(conn: &Connection, project: &Project, author: &str, message: &str) -> Result<bool> {
    let mut snap = project.clone();
    snap.changelog.clear();
    let json = serde_json::to_string(&snap)?;
    let last: Option<String> = conn
        .query_row("SELECT snapshot FROM revisions ORDER BY id DESC LIMIT 1", [], |r| r.get(0))
        .optional()?;
    if last.as_deref() == Some(json.as_str()) {
        return Ok(false);
    }
    conn.execute(
        "INSERT INTO revisions(id, created, author, message, snapshot) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![RevisionId::new().to_string(), now_str(), author, message, json],
    )?;
    conn.execute(
        "DELETE FROM revisions WHERE id NOT IN (SELECT id FROM revisions ORDER BY id DESC LIMIT ?1)",
        params![MAX_REVISIONS],
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::sample_house;

    #[test]
    fn round_trip_with_attachments_and_revisions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("house.abode");

        let mut store = Store::create().unwrap();
        let mut p = sample_house();
        let att = store.add_attachment("manual.pdf", b"%PDF-1.4 fake").unwrap();
        let mut note = Note::new(EntityRef::General);
        note.attachments.push(att.id);
        p.notes.push(note);
        let orphan = store.add_attachment("unused.png", b"nope").unwrap();

        store.save(&p, &path, &p.referenced_attachments(), "me", "first").unwrap();
        store.save(&p, &path, &p.referenced_attachments(), "me", "same again").unwrap();

        let (store2, loaded) = Store::open(&path).unwrap();
        assert_eq!(loaded, p);
        assert_eq!(store2.read_attachment(att.id).unwrap(), b"%PDF-1.4 fake");
        assert!(store2.read_attachment(orphan.id).is_err(), "unreferenced files are dropped");
        assert_eq!(store2.revisions().unwrap().len(), 1, "identical saves don't add revisions");
        let rev = store2.revisions().unwrap()[0].id;
        assert_eq!(store2.load_revision(rev).unwrap().levels, p.levels);
    }

    #[test]
    fn export_strips_private_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("house.abode");
        let shared_path = dir.path().join("for-buyer.abode");

        let mut store = Store::create().unwrap();
        let mut p = sample_house();
        let secret_file = store.add_attachment("alarm.jpg", b"SECRET-BYTES").unwrap();
        let mut secret = Note::new(EntityRef::General);
        secret.title = "Alarm code".into();
        secret.body = "4321".into();
        secret.private = true;
        secret.attachments.push(secret_file.id);
        p.notes.push(secret);
        store.save(&p, &path, &p.referenced_attachments(), "me", "save").unwrap();
        store.export_shared(&p, &shared_path).unwrap();

        let (shared_store, shared) = Store::open(&shared_path).unwrap();
        assert!(shared.notes.iter().all(|n| !n.private));
        assert!(shared_store.read_attachment(secret_file.id).is_err());
        assert!(shared_store.revisions().unwrap().is_empty());
        let raw = fs::read(&shared_path).unwrap();
        let contains = |needle: &[u8]| raw.windows(needle.len()).any(|w| w == needle);
        assert!(!contains(b"SECRET-BYTES"));
        assert!(!contains(b"Alarm code"));
    }

    #[test]
    fn rejects_non_abode_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.abode");
        fs::write(&path, b"hello").unwrap();
        assert!(Store::open(&path).is_err());
    }

    #[test]
    fn peek_reads_the_house_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("h.abode");
        let mut store = Store::create().unwrap();
        let p = crate::sample::sample_house();
        store.save(&p, &path, &Default::default(), "me", "Saved").unwrap();
        let s = peek(&path).unwrap();
        assert_eq!(s.name, "Sample House");
        assert!(s.saved_at.is_some());
    }
}
