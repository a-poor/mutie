//! Explicit, preview-first retention for direct children of a report root.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, Metadata},
    io::{self, ErrorKind, Write},
    num::NonZeroU64,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::{
    cancellation::Cancellation,
    report::RunStatus,
    report_lock,
    report_reader::{LoadedReport, SavedLogs, read_json},
};

const DAY: u64 = 24 * 60 * 60;
const ROOT_FILES: &[&str] = &[
    "run.json",
    "mutants.json",
    "outcomes.json",
    "summary.txt",
    "killed.txt",
    "missed.txt",
    "timeout.txt",
    "build_failed.txt",
    "errors.txt",
];

pub(crate) struct Policy {
    keep_last: usize,
    older_than_seconds: Option<u64>,
}

impl Policy {
    pub(crate) fn new(keep_last: Option<usize>, older_than: Option<NonZeroU64>) -> Result<Self> {
        ensure!(
            keep_last.is_some() || older_than.is_some(),
            "a retention rule is required"
        );
        let older_than_seconds = older_than
            .map(|days| {
                days.get()
                    .checked_mul(DAY)
                    .context("--older-than is too large")
            })
            .transpose()?;
        Ok(Self {
            keep_last: keep_last.unwrap_or(0),
            older_than_seconds,
        })
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Header {
    mutie_version: String,
    checkpoint: u64,
    status: RunStatus,
    started_at_unix_seconds: u64,
    finished_at_unix_seconds: Option<u64>,
}

impl Header {
    fn timestamp(&self) -> u64 {
        self.finished_at_unix_seconds
            .unwrap_or(self.started_at_unix_seconds)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Identity {
    created: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl Identity {
    fn new(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            created: metadata.created().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Artifact {
    identity: Identity,
    bytes: u64,
    modified: Option<SystemTime>,
    directory: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    identity: Identity,
    files: BTreeMap<PathBuf, Artifact>,
}

struct Entry {
    path: PathBuf,
    header: Option<Header>,
    bytes: Option<u64>,
    snapshot: Option<Snapshot>,
    protected: Option<String>,
    selected: bool,
}

/// Inspect only Mutie's flat artifact directories. Never traverse a symlink or
/// an unexpected directory, even to calculate its size.
fn snapshot(directory: &Path, cancellation: &Cancellation) -> Result<Snapshot> {
    let metadata = fs::symlink_metadata(directory)?;
    ensure!(
        metadata.is_dir(),
        "not a regular run directory (symlinks are protected)"
    );
    let mut snapshot = Snapshot {
        identity: Identity::new(&metadata),
        files: BTreeMap::new(),
    };
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        for item in fs::read_dir(directory.join(&relative))? {
            cancellation.check()?;
            let item = item?;
            let path = relative.join(item.file_name());
            let metadata = fs::symlink_metadata(item.path())?;
            if path == Path::new(report_lock::NAME) {
                ensure!(
                    metadata.is_file() && metadata.len() == 0,
                    "unexpected report lock contents or type"
                );
                continue;
            }
            ensure!(
                metadata.is_file() || metadata.is_dir(),
                "symlink or special file at {path:?}"
            );
            if metadata.is_dir() {
                ensure!(
                    path == Path::new("diff") || path == Path::new("logs"),
                    "unexpected directory {path:?}"
                );
                pending.push(path.clone());
            }
            snapshot.files.insert(
                path,
                Artifact {
                    identity: Identity::new(&metadata),
                    bytes: if metadata.is_file() {
                        metadata.len()
                    } else {
                        0
                    },
                    modified: metadata.modified().ok(),
                    directory: metadata.is_dir(),
                },
            );
        }
    }
    Ok(snapshot)
}

fn check_logs(logs: &SavedLogs, prefix: &str) -> Result<()> {
    for (path, suffix) in [
        (&logs.text, ".log"),
        (&logs.stdout, ".stdout.jsonl"),
        (&logs.stderr, ".stderr.log"),
    ] {
        if let Some(path) = path {
            ensure!(
                path == Path::new(&format!("logs/{prefix}{suffix}")),
                "unexpected log reference {path:?}"
            );
        }
    }
    Ok(())
}

fn validate_layout(report: &LoadedReport, snapshot: &Snapshot) -> Result<()> {
    let mut allowed: BTreeSet<PathBuf> = ROOT_FILES.iter().map(PathBuf::from).collect();
    let mut logs = vec!["baseline".to_owned(), "discovery".to_owned()];
    for mutation in &report.mutations {
        let diff = PathBuf::from(format!("diff/{:06}.diff", mutation.id));
        if let Some(path) = &mutation.diff {
            ensure!(*path == diff, "unexpected diff reference {path:?}");
        }
        allowed.insert(diff);
        logs.push(format!("{:06}", mutation.id));
    }
    for prefix in logs {
        for suffix in [".log", ".stdout.jsonl", ".stderr.log"] {
            allowed.insert(PathBuf::from(format!("logs/{prefix}{suffix}")));
        }
    }
    for (prefix, command) in [
        ("baseline", &report.baseline),
        ("discovery", &report.discovery),
    ] {
        if let Some(command) = command {
            check_logs(&command.logs, prefix)?;
        }
    }
    for execution in &report.executions {
        if let Some(logs) = &execution.logs {
            check_logs(logs, &format!("{:06}", execution.id))?;
        }
    }
    for (path, artifact) in &snapshot.files {
        if !artifact.directory {
            ensure!(allowed.contains(path), "unexpected file {path:?}");
        }
    }
    for name in ["diff", "logs"] {
        ensure!(
            snapshot
                .files
                .get(Path::new(name))
                .is_some_and(|item| item.directory),
            "missing {name} directory"
        );
    }
    Ok(())
}

fn inspect(path: PathBuf, now: u64, cancellation: &Cancellation, locked: bool) -> Entry {
    let mut entry = Entry {
        path,
        header: None,
        bytes: None,
        snapshot: None,
        protected: None,
        selected: false,
    };
    let result = (|| -> Result<()> {
        let snapshot = snapshot(&entry.path, cancellation)?;
        entry.bytes = Some(snapshot.files.values().try_fold(0u64, |sum, item| {
            sum.checked_add(item.bytes).context("report size overflow")
        })?);
        entry.snapshot = Some(snapshot);
        let header: Header = read_json(&entry.path.canonicalize()?, "run.json")?;
        entry.header = Some(header);
        let header = entry.header.as_ref().unwrap();
        let _lock = if locked {
            None
        } else {
            report_lock::acquire(&entry.path, false)?
        };
        ensure!(
            header.status != RunStatus::Running,
            "run is not finalized (including abandoned running reports)"
        );
        let finished = header
            .finished_at_unix_seconds
            .context("missing finish time")?;
        ensure!(!header.mutie_version.is_empty(), "missing Mutie version");
        ensure!(
            finished >= header.started_at_unix_seconds && finished <= now,
            "invalid or future report timestamps"
        );
        let report = LoadedReport::load(&entry.path)?;
        validate_layout(&report, entry.snapshot.as_ref().unwrap())?;
        Ok(())
    })();
    entry.protected = result.err().map(|error| format!("{error:#}"));
    entry
}

fn scan(directory: &Path, now: u64, cancellation: &Cancellation) -> Result<(Vec<Entry>, usize)> {
    cancellation.check()?;
    let items = match fs::read_dir(directory) {
        Ok(items) => items,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((Vec::new(), 0)),
        Err(error) => return Err(error).context("opening report root"),
    };
    let mut entries = Vec::new();
    let mut unrelated = 0;
    for item in items {
        cancellation.check()?;
        let item = item?;
        if item
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("run-"))
        {
            entries.push(inspect(item.path(), now, cancellation, false));
        } else {
            unrelated += 1;
        }
    }
    cancellation.check()?;
    entries.sort_by(|a, b| {
        b.header
            .as_ref()
            .map(Header::timestamp)
            .cmp(&a.header.as_ref().map(Header::timestamp))
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok((entries, unrelated))
}

fn select(entries: &mut [Entry], policy: &Policy, now: u64) {
    let mut eligible = 0;
    for entry in entries {
        entry.selected = false;
        if entry.protected.is_some() {
            continue;
        }
        entry.selected = eligible >= policy.keep_last
            && policy.older_than_seconds.is_none_or(|age| {
                now.saturating_sub(entry.header.as_ref().unwrap().timestamp()) > age
            });
        eligible += 1;
    }
}

fn remove(entry: &Entry, now: u64, cancellation: &Cancellation) -> Result<()> {
    cancellation.check()?;
    ensure!(
        entry.selected && entry.protected.is_none(),
        "run was not selected for pruning"
    );
    // Before creating a legacy lock, reject moved directories and symlinks.
    ensure!(
        snapshot(&entry.path, cancellation)? == *entry.snapshot.as_ref().unwrap(),
        "report changed since selection"
    );
    let lock = report_lock::acquire(&entry.path, true)?.context("report lock disappeared")?;
    let current = inspect(entry.path.clone(), now, cancellation, true);
    cancellation.check()?;
    ensure!(
        current.protected.is_none(),
        "report is now protected: {}",
        current.protected.as_deref().unwrap_or_default()
    );
    ensure!(
        entry.header == current.header && entry.snapshot == current.snapshot,
        "report changed since selection"
    );
    cancellation.check()?;
    // Keep the lock through deletion. Do not interrupt midway through a run.
    remove_artifacts(&entry.path, current.snapshot.as_ref().unwrap(), &lock)
        .context("removing report (some files may already have been removed)")
}

fn remove_artifacts(directory: &Path, snapshot: &Snapshot, lock: &fs::File) -> Result<()> {
    // Delete only the validated file list. Recursive deletion could consume a
    // user's file added after validation; remove_dir instead preserves it and
    // reports an error. Keep the persistent lock until all artifacts are gone.
    for (relative, artifact) in &snapshot.files {
        if artifact.directory {
            continue;
        }
        let path = directory.join(relative);
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            metadata.is_file()
                && Identity::new(&metadata) == artifact.identity
                && metadata.len() == artifact.bytes
                && metadata.modified().ok() == artifact.modified,
            "artifact changed during deletion: {relative:?}"
        );
        fs::remove_file(path)?;
    }
    for (relative, artifact) in &snapshot.files {
        if artifact.directory {
            fs::remove_dir(directory.join(relative))?;
        }
    }
    let path = directory.join(report_lock::NAME);
    let metadata = fs::symlink_metadata(&path)?;
    ensure!(
        metadata.is_file()
            && metadata.len() == 0
            && Identity::new(&metadata) == Identity::new(&lock.metadata()?),
        "report lock changed during deletion"
    );
    fs::remove_file(path)?;
    fs::remove_dir(directory)?;
    Ok(())
}

fn size(bytes: u64) -> String {
    for (unit, divisor) in [("GiB", 1u64 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)] {
        if bytes >= divisor {
            return format!("{:.1} {unit}", bytes as f64 / divisor as f64);
        }
    }
    format!("{bytes} B")
}

fn age(timestamp: u64, now: u64) -> String {
    if timestamp > now {
        return "future".into();
    }
    let seconds = now - timestamp;
    if seconds >= DAY {
        format!("{}d", seconds / DAY)
    } else if seconds >= 3600 {
        format!("{}h", seconds / 3600)
    } else if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

pub(crate) fn run(
    directory: &Path,
    policy: Option<Policy>,
    apply: bool,
    cancellation: &Cancellation,
) -> Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let (mut entries, unrelated) = scan(directory, now, cancellation)?;
    if let Some(policy) = &policy {
        select(&mut entries, policy, now);
    }
    let stdout = io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "Saved runs in {directory:?}")?;
    writeln!(
        out,
        "{:<10} {:<11} {:>7} {:>12}  RUN",
        "ACTION", "STATUS", "AGE", "SIZE"
    )?;
    for entry in &entries {
        let action = if entry.protected.is_some() {
            "protected"
        } else if entry.selected {
            "prune"
        } else {
            "keep"
        };
        let status = entry
            .header
            .as_ref()
            .map(|h| h.status.to_string())
            .unwrap_or_else(|| "unknown".into());
        let age = entry
            .header
            .as_ref()
            .map(|h| age(h.timestamp(), now))
            .unwrap_or_else(|| "?".into());
        let bytes = entry.bytes.map(size).unwrap_or_else(|| "?".into());
        writeln!(
            out,
            "{action:<10} {status:<11} {age:>7} {bytes:>12}  {:?}",
            entry.path.file_name().unwrap()
        )?;
        if let Some(reason) = &entry.protected {
            writeln!(out, "  Reason: {reason:?}")?;
        }
    }
    if entries.is_empty() {
        writeln!(out, "No saved runs found.")?;
    }
    writeln!(
        out,
        "Sizes are logical file bytes; ? means unavailable. Age is since finish, or start for running runs."
    )?;
    if unrelated != 0 {
        writeln!(out, "Ignored {unrelated} unrelated root entries.")?;
    }
    if policy.is_none() {
        return Ok(());
    }
    let count = entries.iter().filter(|entry| entry.selected).count();
    let bytes: u128 = entries
        .iter()
        .filter(|entry| entry.selected)
        .map(|entry| entry.bytes.unwrap() as u128)
        .sum();
    writeln!(out, "Selected runs: {count} ({bytes} logical bytes).")?;
    if !apply {
        writeln!(
            out,
            "Preview only; nothing changed. Add --apply to delete selected runs."
        )?;
        return Ok(());
    }
    out.flush()?;
    let mut deleted = 0;
    let mut failures = 0;
    for entry in entries.iter().filter(|entry| entry.selected) {
        cancellation
            .check()
            .with_context(|| format!("pruning stopped after deleting {deleted} runs"))?;
        match remove(entry, now, cancellation) {
            Ok(()) => {
                deleted += 1;
                writeln!(out, "Deleted {:?}", entry.path.file_name().unwrap())?;
            }
            Err(error) => {
                failures += 1;
                writeln!(
                    out,
                    "Could not delete {:?}: {:?}",
                    entry.path.file_name().unwrap(),
                    format!("{error:#}")
                )?;
            }
        }
    }
    cancellation
        .check()
        .with_context(|| format!("pruning stopped after deleting {deleted} runs"))?;
    writeln!(out, "Deleted {deleted} of {count} selected runs.")?;
    ensure!(
        failures == 0,
        "{failures} selected runs could not be fully removed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cli::Cli, config::RunConfig, report_reader, saved_report::SavedReport};
    use clap::Parser;
    use serde_json::{Value, json};

    const NOW: u64 = 100 * DAY;

    fn edit(path: &Path, name: &str, change: impl FnOnce(&mut Value)) {
        let mut value: Value = serde_json::from_slice(&fs::read(path.join(name)).unwrap()).unwrap();
        change(&mut value);
        fs::write(path.join(name), serde_json::to_vec(&value).unwrap()).unwrap();
    }

    fn fixture(root: &Path, name: &str, finished: u64) -> PathBuf {
        let source = report_reader::fixture();
        let path = root.join(name);
        fs::rename(source.path(), &path).unwrap();
        edit(&path, "run.json", |run| {
            run["mutie_version"] = json!(env!("CARGO_PKG_VERSION"));
            run["started_at_unix_seconds"] = json!(finished - 1);
            run["finished_at_unix_seconds"] = json!(finished);
        });
        path
    }

    fn plan(root: &Path, keep: Option<usize>, days: Option<u64>) -> Vec<Entry> {
        let (mut entries, _) = scan(root, NOW, &Cancellation::default()).unwrap();
        select(
            &mut entries,
            &Policy::new(keep, days.and_then(NonZeroU64::new)).unwrap(),
            NOW,
        );
        entries
    }

    #[test]
    fn rules_intersect_age_is_strict_and_ties_are_deterministic() {
        let root = tempfile::tempdir().unwrap();
        fixture(root.path(), "run-recent", NOW - DAY);
        fixture(root.path(), "run-at-cutoff", NOW - 7 * DAY);
        fixture(root.path(), "run-old-b", NOW - 8 * DAY);
        fixture(root.path(), "run-old-a", NOW - 8 * DAY);
        let entries = plan(root.path(), Some(1), Some(7));
        let names: Vec<_> = entries
            .iter()
            .filter(|e| e.selected)
            .map(|e| e.path.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, ["run-old-a", "run-old-b"]);
        let entries = plan(root.path(), Some(3), Some(7));
        assert_eq!(entries.iter().filter(|e| e.selected).count(), 1);
        assert!(
            entries
                .iter()
                .find(|e| e.path.ends_with("run-old-b"))
                .unwrap()
                .selected
        );
        assert!(
            plan(root.path(), Some(10), None)
                .iter()
                .all(|e| !e.selected)
        );
        assert!(plan(root.path(), Some(0), None).iter().all(|e| e.selected));
        assert_eq!(
            plan(root.path(), None, Some(7))
                .iter()
                .filter(|e| e.selected)
                .count(),
            2
        );
        assert!(Policy::new(None, None).is_err());
        assert!(Policy::new(None, NonZeroU64::new(u64::MAX)).is_err());
    }

    #[test]
    fn preview_is_read_only_and_apply_removes_only_selected_reports() {
        let root = tempfile::tempdir().unwrap();
        let old = fixture(root.path(), "run-old", NOW - 2 * DAY);
        let new = fixture(root.path(), "run-new", NOW - DAY);
        fs::write(root.path().join("notes.txt"), "keep me").unwrap();
        let cancel = Cancellation::default();
        let before = snapshot(&old, &cancel).unwrap();
        let entries = plan(root.path(), Some(1), None);
        assert_eq!(snapshot(&old, &cancel).unwrap(), before);
        assert!(!old.join(report_lock::NAME).exists());
        let selected = entries.iter().find(|entry| entry.selected).unwrap();
        assert_eq!(
            selected.bytes.unwrap(),
            before.files.values().map(|file| file.bytes).sum::<u64>()
        );
        remove(selected, NOW, &cancel).unwrap();
        assert!(!old.exists());
        assert!(new.exists());
        assert_eq!(
            fs::read_to_string(root.path().join("notes.txt")).unwrap(),
            "keep me"
        );
        assert_eq!(scan(root.path(), NOW, &cancel).unwrap().1, 1);
    }

    #[test]
    fn protects_invalid_unfinished_and_unexpected_contents() {
        let root = tempfile::tempdir().unwrap();
        let pending = fixture(root.path(), "run-pending", NOW);
        edit(&pending, "run.json", |r| {
            r["status"] = json!("running");
            r["finished_at_unix_seconds"] = Value::Null;
        });
        let inconsistent = fixture(root.path(), "run-inconsistent", NOW);
        edit(&inconsistent, "outcomes.json", |r| {
            r["checkpoint"] = json!(9)
        });
        let unsupported = fixture(root.path(), "run-unsupported", NOW);
        edit(&unsupported, "run.json", |r| {
            r["schema_version"] = json!(99)
        });
        let extra = fixture(root.path(), "run-extra", NOW);
        fs::write(extra.join("notes.txt"), "personal notes").unwrap();
        let nested = fixture(root.path(), "run-nested", NOW);
        fs::create_dir(nested.join("logs/notes")).unwrap();
        let unknown_log = fixture(root.path(), "run-unknown-log", NOW);
        fs::write(unknown_log.join("logs/000999.log"), "unknown mutation").unwrap();
        let traversal = fixture(root.path(), "run-traversal", NOW);
        edit(&traversal, "outcomes.json", |r| {
            r["baseline"]["logs"]["stdout"] = json!("../other.jsonl")
        });
        fixture(root.path(), "run-future", NOW + 1);
        let backwards = fixture(root.path(), "run-backwards", NOW);
        edit(&backwards, "run.json", |r| {
            r["started_at_unix_seconds"] = json!(NOW + 1)
        });
        let incomplete = root.path().join("run-incomplete");
        fs::create_dir(&incomplete).unwrap();
        fs::write(incomplete.join("run.json"), "{").unwrap();
        let entries = plan(root.path(), Some(0), None);
        assert_eq!(entries.len(), 10);
        for entry in entries {
            assert!(!entry.selected, "{:?}", entry.path);
            assert!(entry.protected.is_some(), "{:?}", entry.path);
            assert!(entry.path.exists());
        }
    }

    #[test]
    fn protected_runs_do_not_consume_keep_slots() {
        let root = tempfile::tempdir().unwrap();
        let active = fixture(root.path(), "run-active", NOW);
        edit(&active, "run.json", |r| r["status"] = json!("running"));
        fixture(root.path(), "run-new", NOW - DAY);
        fixture(root.path(), "run-old", NOW - 2 * DAY);
        let entries = plan(root.path(), Some(1), None);
        assert_eq!(entries.iter().filter(|e| e.selected).count(), 1);
        assert!(
            entries
                .iter()
                .find(|e| e.path.ends_with("run-old"))
                .unwrap()
                .selected
        );
    }

    #[test]
    fn writer_lock_protects_even_a_finalized_report_until_drop() {
        let module = tempfile::tempdir().unwrap();
        fs::write(module.path().join("go.mod"), "module example.com/test\n").unwrap();
        let cli = Cli::parse_from([std::ffi::OsStr::new("mutie"), module.path().as_os_str()]);
        let config = RunConfig::resolve(cli).unwrap();
        let mut report = SavedReport::create(&config).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let path = report.path().to_owned();
        let cancel = Cancellation::default();
        assert!(
            inspect(path.clone(), now, &cancel, false)
                .protected
                .unwrap()
                .contains("locked")
        );
        report
            .finish(Some(&anyhow::anyhow!("baseline failed")))
            .unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(
            LoadedReport::load(&path).unwrap().run.status,
            RunStatus::Failed
        );
        assert!(
            inspect(path.clone(), now, &cancel, false)
                .protected
                .unwrap()
                .contains("locked")
        );
        drop(report);
        let mut entries = scan(&config.output_dir, now, &cancel).unwrap().0;
        select(&mut entries, &Policy::new(Some(0), None).unwrap(), now);
        assert!(entries[0].selected);
        remove(&entries[0], now, &cancel).unwrap();
        assert!(!path.exists());
        assert!(module.path().join("go.mod").exists());
    }

    #[test]
    fn revalidation_preserves_changed_replaced_and_locked_runs() {
        let root = tempfile::tempdir().unwrap();
        let cancel = Cancellation::default();
        let changed = fixture(root.path(), "run-changed", NOW);
        let entries = plan(root.path(), Some(0), None);
        fs::write(changed.join("notes.txt"), "added after preview").unwrap();
        assert!(remove(&entries[0], NOW, &cancel).is_err());
        assert!(!changed.join(report_lock::NAME).exists());
        let replaced = fixture(root.path(), "run-replaced", NOW);
        let entries = plan(root.path(), Some(0), None);
        let entry = entries.iter().find(|e| e.path == replaced).unwrap();
        fs::rename(&replaced, root.path().join("original")).unwrap();
        fixture(root.path(), "run-replaced", NOW);
        assert!(remove(entry, NOW, &cancel).is_err());
        assert!(replaced.exists());
        let locked = fixture(root.path(), "run-locked", NOW);
        let entries = plan(root.path(), Some(0), None);
        let entry = entries.iter().find(|e| e.path == locked).unwrap();
        let guard = report_lock::acquire(&locked, true).unwrap().unwrap();
        assert!(
            remove(entry, NOW, &cancel)
                .unwrap_err()
                .to_string()
                .contains("locked")
        );
        drop(guard);
        remove(entry, NOW, &cancel).unwrap();
        // A second pruner must not treat a disappearing selected run as success.
        assert!(remove(entry, NOW, &cancel).is_err());
    }

    #[test]
    fn cancellation_preserves_selected_reports_and_missing_roots_stay_missing() {
        let root = tempfile::tempdir().unwrap();
        let path = fixture(root.path(), "run-old", NOW);
        let entries = plan(root.path(), Some(0), None);
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(remove(&entries[0], NOW, &cancel).is_err());
        assert!(scan(root.path(), NOW, &cancel).is_err());
        assert!(path.exists());
        let missing = root.path().join("missing");
        assert!(
            scan(&missing, NOW, &Cancellation::default())
                .unwrap()
                .0
                .is_empty()
        );
        assert!(!missing.exists());
    }

    #[test]
    fn measures_large_logs_without_reading_them_and_rejects_later_metadata_changes() {
        let root = tempfile::tempdir().unwrap();
        let path = fixture(root.path(), "run-large", NOW);
        let length = 5 * (1u64 << 30);
        fs::OpenOptions::new()
            .write(true)
            .open(path.join("logs/000001.log"))
            .unwrap()
            .set_len(length)
            .unwrap();
        let entries = plan(root.path(), Some(0), None);
        assert!(entries[0].selected);
        assert!(entries[0].bytes.unwrap() > length);
        edit(&path, "run.json", |r| r["checkpoint"] = json!(11));
        assert!(remove(&entries[0], NOW, &Cancellation::default()).is_err());
        assert!(path.exists());
    }

    #[test]
    fn deletion_preserves_unexpected_files_added_after_validation() {
        let root = tempfile::tempdir().unwrap();
        for (name, relative) in [
            ("run-extra", "notes.txt"),
            ("run-log-extra", "logs/notes.txt"),
        ] {
            let path = fixture(root.path(), name, NOW);
            let lock = report_lock::acquire(&path, true).unwrap().unwrap();
            let inspected = inspect(path.clone(), NOW, &Cancellation::default(), true);
            assert!(inspected.protected.is_none());
            fs::write(path.join(relative), "added during deletion").unwrap();
            assert!(remove_artifacts(&path, inspected.snapshot.as_ref().unwrap(), &lock).is_err());
            assert_eq!(
                fs::read_to_string(path.join(relative)).unwrap(),
                "added during deletion"
            );
            assert!(path.exists());
        }
    }

    #[cfg(unix)]
    #[test]
    fn never_traverses_symlinks_at_any_report_level() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = fixture(outside.path(), "run-target", NOW);
        let before = snapshot(&target, &Cancellation::default()).unwrap();
        symlink(&target, root.path().join("run-linked")).unwrap();
        for (name, relative, destination) in [
            (
                "run-log-link",
                "logs/extra.log",
                target.join("logs/baseline.log"),
            ),
            ("run-metadata-link", "run.json", target.join("run.json")),
            ("run-directory-link", "logs", target.join("logs")),
            ("run-lock-link", report_lock::NAME, target.join("run.json")),
        ] {
            let run = fixture(root.path(), name, NOW);
            let path = run.join(relative);
            if path.is_dir() {
                fs::remove_dir_all(&path).unwrap();
            } else if path.exists() {
                fs::remove_file(&path).unwrap();
            }
            symlink(destination, path).unwrap();
        }
        for entry in plan(root.path(), Some(0), None) {
            assert!(entry.protected.is_some());
            assert!(!entry.selected);
        }
        assert_eq!(snapshot(&target, &Cancellation::default()).unwrap(), before);
        // Replacement with a symlink after preview is also protected.
        let path = fixture(root.path(), "run-swapped", NOW);
        let entries = plan(root.path(), Some(0), None);
        let entry = entries.iter().find(|e| e.path == path).unwrap();
        fs::rename(&path, root.path().join("moved")).unwrap();
        symlink(&target, &path).unwrap();
        assert!(remove(entry, NOW, &Cancellation::default()).is_err());
        assert!(target.exists());
    }
}
