//! Repository-owned review decisions. These never change execution or saved outcomes.
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::{
    cancellation::{Cancellation, Interrupted},
    cli::{DismissAction, Lang},
    config::RunConfig,
    identity::MutationIdentity,
    mutation::{self, Mutation, OPERATOR_CATALOG_VERSION, Scope},
    report_reader::{LoadedReport, SavedMutation},
};

const FILE_NAME: &str = "mutie.dismissals.toml";
const FORMAT_VERSION: u32 = 1;
const LOCK_NAME: &str = ".mutie.dismissals.lock";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dismissal {
    pub id: usize,
    #[serde(default = "legacy_language")]
    pub language: Lang,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub mutation: MutationIdentity,
}

fn legacy_language() -> Lang {
    Lang::Go
}

impl Dismissal {
    fn validate(&self) -> Result<()> {
        ensure!(self.id > 0, "dismissal ID must be positive");
        ensure!(
            self.language != Lang::Auto,
            "dismissal language must be go or rust"
        );
        validate_text(&self.reason, "reason")?;
        if let Some(note) = &self.note {
            validate_text(note, "note")?;
        }
        self.mutation
            .validate()
            .with_context(|| format!("dismissal {}", self.id))
    }

    pub fn description(&self) -> String {
        let mut text = format!(
            "Dismissal {}: {}\nReason: {}\n",
            self.id,
            self.mutation.file.display(),
            self.reason
        );
        if let Some(note) = &self.note {
            text.push_str(&format!("Note: {note}\n"));
        }
        crate::report_reader::clean_text(&text)
    }
}

/// Review decisions captured against the run's pristine source, never live repository state.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DismissalSnapshot {
    pub matched: Vec<MatchedDismissal>,
    pub needs_review: Vec<DismissalIssue>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MatchedDismissal {
    pub mutation_id: usize,
    pub dismissal: Dismissal,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DismissalIssue {
    pub status: MatchStatus,
    pub dismissal: Dismissal,
}

impl DismissalSnapshot {
    pub fn dismissal(&self, id: usize) -> Option<&Dismissal> {
        self.matched
            .binary_search_by_key(&id, |m| m.mutation_id)
            .ok()
            .map(|index| &self.matched[index].dismissal)
    }

    /// Rerun selection renumbers mutations; omitted valid decisions are not stale.
    pub fn select(&mut self, mutations: &[Mutation]) {
        let ids: BTreeMap<_, _> = mutations
            .iter()
            .enumerate()
            .map(|(i, m)| (MutationIdentity::from(m), i + 1))
            .collect();
        self.matched.retain_mut(|entry| {
            if let Some(id) = ids.get(&entry.dismissal.mutation) {
                entry.mutation_id = *id;
                true
            } else {
                false
            }
        });
        self.matched.sort_by_key(|entry| entry.mutation_id);
    }

    pub fn validate(
        &mut self,
        language: Lang,
        mutations: &BTreeMap<usize, SavedMutation>,
    ) -> Result<()> {
        let mut ids = BTreeSet::new();
        let mut targets = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for entry in &self.matched {
            entry.dismissal.validate()?;
            ensure!(
                entry.dismissal.language == language,
                "matched dismissal language differs from saved run"
            );
            ensure!(
                ids.insert(entry.dismissal.id),
                "duplicate saved dismissal ID {}",
                entry.dismissal.id
            );
            ensure!(
                targets.insert(entry.mutation_id),
                "multiple dismissals for saved mutation {}",
                entry.mutation_id
            );
            ensure!(
                identities.insert(&entry.dismissal.mutation),
                "ambiguous saved dismissal identity matches multiple mutations"
            );
            let mutation = mutations
                .get(&entry.mutation_id)
                .context("dismissal references unknown saved mutation")?;
            ensure!(
                MutationIdentity::from(mutation) == entry.dismissal.mutation,
                "dismissal identity does not match saved mutation {}",
                entry.mutation_id
            );
        }
        for entry in &self.needs_review {
            entry.dismissal.validate()?;
            ensure!(
                (entry.dismissal.language != language)
                    == (entry.status == MatchStatus::LanguageMismatch),
                "dismissal language review disagrees with saved run"
            );
            ensure!(
                ids.insert(entry.dismissal.id),
                "duplicate saved dismissal ID {}",
                entry.dismissal.id
            );
            ensure!(
                !matches!(entry.status, MatchStatus::Matched(_)),
                "review issue cannot be a matched dismissal"
            );
            ensure!(
                !self
                    .matched
                    .iter()
                    .any(|m| m.dismissal.language == entry.dismissal.language
                        && m.dismissal.mutation == entry.dismissal.mutation),
                "ambiguous saved dismissal identity appears as matched and needing review"
            );
        }
        self.matched.sort_by_key(|entry| entry.mutation_id);
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Store {
    format_version: u32,
    operator_catalog_version: u32,
    next_id: usize,
    dismissals: Vec<Dismissal>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            operator_catalog_version: OPERATOR_CATALOG_VERSION,
            next_id: 1,
            dismissals: Vec::new(),
        }
    }
}

impl Store {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(FILE_NAME);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error).with_context(|| format!("opening {}", path.display())),
            Ok(metadata) => ensure!(
                metadata.is_file(),
                "{} must be a regular file",
                path.display()
            ),
        }
        let source =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let mut store: Self =
            toml::from_str(&source).with_context(|| format!("parsing {}", path.display()))?;
        store
            .validate()
            .with_context(|| format!("validating {}", path.display()))?;
        store.dismissals.sort_by_key(|d| d.id);
        Ok(store)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.format_version == FORMAT_VERSION,
            "unsupported dismissal format version {}",
            self.format_version
        );
        ensure!(
            self.operator_catalog_version == OPERATOR_CATALOG_VERSION,
            "dismissals use an incompatible operator catalog"
        );
        ensure!(self.next_id > 0, "next_id must be positive");
        let mut ids = BTreeSet::new();
        for entry in &self.dismissals {
            ensure!(
                entry.id > 0 && entry.id < self.next_id,
                "invalid dismissal ID {}; next_id must exceed all stored IDs",
                entry.id
            );
            ensure!(ids.insert(entry.id), "duplicate dismissal ID {}", entry.id);
            entry.validate()?;
        }
        Ok(())
    }

    pub fn snapshot(
        &self,
        language: Lang,
        mutations: &[Mutation],
        cancellation: &Cancellation,
    ) -> Result<DismissalSnapshot> {
        let mut snapshot = DismissalSnapshot::default();
        for (entry, status) in self
            .dismissals
            .iter()
            .zip(match_dismissals(self, language, mutations))
        {
            cancellation.check()?;
            let dismissal = entry.clone();
            match status {
                MatchStatus::Matched(mutation_id) => snapshot.matched.push(MatchedDismissal {
                    mutation_id,
                    dismissal,
                }),
                _ => snapshot
                    .needs_review
                    .push(DismissalIssue { status, dismissal }),
            }
        }
        snapshot.matched.sort_by_key(|entry| entry.mutation_id);
        Ok(snapshot)
    }

    fn save(&self, root: &Path) -> Result<()> {
        self.validate()?;
        let contents = toml::to_string_pretty(self).context("serializing dismissals")?;
        let path = root.join(FILE_NAME);
        let mut file = NamedTempFile::new_in(root).context("preparing dismissal file")?;
        file.write_all(contents.as_bytes())
            .context("writing dismissals")?;
        file.as_file().sync_all().context("flushing dismissals")?;
        file.persist(&path)
            .with_context(|| format!("saving {}", path.display()))?;
        Ok(())
    }

    fn add(
        &mut self,
        language: Lang,
        mutation: MutationIdentity,
        reason: String,
        note: Option<String>,
    ) -> Result<usize> {
        if let Some(existing) = self
            .dismissals
            .iter()
            .find(|d| d.language == language && d.mutation == mutation)
        {
            anyhow::bail!(
                "mutation already has dismissal {}; use dismiss update",
                existing.id
            );
        }
        let id = self.next_id;
        self.next_id = id.checked_add(1).context("dismissal IDs exhausted")?;
        self.dismissals.push(Dismissal {
            id,
            language,
            reason,
            note,
            mutation,
        });
        Ok(id)
    }
}

fn validate_text(text: &str, field: &str) -> Result<()> {
    ensure!(
        !text.trim().is_empty(),
        "dismissal {field} must not be blank"
    );
    Ok(())
}

// Serialize CLI writers across their entire read/modify/replace transaction.
// The atomic replacement also lets readers see a complete old or new document.
struct WriteLock(PathBuf);

impl WriteLock {
    fn acquire(root: &Path) -> Result<Self> {
        let path = root.join(LOCK_NAME);
        OpenOptions::new().write(true).create_new(true).open(&path)
            .with_context(|| format!("locking dismissals with {}; if a previous writer was interrupted, remove this lock after confirming no dismissal command is running", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn manage(module: &Path, requested: Lang, action: DismissAction) -> Result<()> {
    let root = module
        .canonicalize()
        .with_context(|| format!("opening {}", module.display()))?;
    let language = crate::language::resolve(&root, requested)?;
    ensure!(
        language != Lang::Vitest,
        "Vitest dismissal review is not supported yet"
    );
    if matches!(action, DismissAction::List) {
        let store = Store::load(&root)?;
        println!(
            "{} dismissal(s) in {} (current source unchecked)",
            store.dismissals.len(),
            root.join(FILE_NAME).display()
        );
        for entry in store.dismissals {
            let m = &entry.mutation;
            println!(
                "\nDismissal {}: {:?}:{}:{}  {}",
                entry.id, m.file, m.line, m.column, m.operator
            );
            println!("  {:?} -> {:?}", m.original, m.replacement);
            println!("  Reason: {:?}", entry.reason);
            println!("  Language: {:?}", entry.language);
            if let Some(note) = entry.note {
                println!("  Note: {note:?}");
            }
            println!(
                "  Source: {}; bytes {}..{}; end {}:{}",
                m.source_fingerprint, m.byte_start, m.byte_end, m.end_line, m.end_column
            );
        }
        return Ok(());
    }
    let _lock = WriteLock::acquire(&root)?;
    let mut store = Store::load(&root)?;
    let message = match action {
        DismissAction::Add {
            run_dir,
            id,
            reason,
            note,
        } => {
            let report = LoadedReport::load(&run_dir)?;
            ensure!(
                report.run.language() == language,
                "saved run language {:?} does not match project language {:?}",
                report.run.language(),
                language
            );
            ensure!(
                report.complete,
                "saved run has an incomplete mutation inventory"
            );
            let mutation = report
                .mutations
                .iter()
                .find(|m| m.id == id.get())
                .with_context(|| format!("unknown source mutation ID {id}"))?;
            let dismissal_id =
                store.add(language, MutationIdentity::from(mutation), reason, note)?;
            format!(
                "Added dismissal {dismissal_id} from saved mutation {id}. Current source unchecked; use dismiss check."
            )
        }
        DismissAction::Update {
            id,
            reason,
            note,
            clear_note,
        } => {
            ensure!(
                reason.is_some() || note.is_some() || clear_note,
                "update requires --reason, --note, or --clear-note"
            );
            let entry = store
                .dismissals
                .iter_mut()
                .find(|d| d.id == id.get())
                .with_context(|| format!("unknown dismissal ID {id}"))?;
            if let Some(reason) = reason {
                entry.reason = reason;
            }
            if clear_note {
                entry.note = None;
            } else if let Some(note) = note {
                entry.note = Some(note);
            }
            format!("Updated dismissal {id}.")
        }
        DismissAction::Remove { id } => {
            let index = store
                .dismissals
                .iter()
                .position(|d| d.id == id.get())
                .with_context(|| format!("unknown dismissal ID {id}"))?;
            store.dismissals.remove(index);
            format!("Removed dismissal {id}.")
        }
        DismissAction::List | DismissAction::Check => {
            unreachable!("read-only commands are handled separately")
        }
    };
    store.save(&root)?;
    println!("{message}\nSaved {}", root.join(FILE_NAME).display());
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Matched(usize),
    Missing,
    Stale,
    Unmatched,
    Ambiguous,
    LanguageMismatch,
}

impl MatchStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Matched(_) => "matched",
            Self::Missing => "missing",
            Self::Stale => "stale",
            Self::Unmatched => "unmatched",
            Self::Ambiguous => "ambiguous",
            Self::LanguageMismatch => "language mismatch",
        }
    }
}

fn match_dismissals(store: &Store, language: Lang, mutations: &[Mutation]) -> Vec<MatchStatus> {
    let mut by_file: HashMap<&Path, Vec<(usize, &Mutation)>> = HashMap::new();
    for (index, mutation) in mutations.iter().enumerate() {
        by_file
            .entry(&mutation.file.path)
            .or_default()
            .push((index + 1, mutation));
    }
    let mut statuses: Vec<_> = store
        .dismissals
        .iter()
        .map(|entry| {
            if entry.language != language {
                return MatchStatus::LanguageMismatch;
            }
            let target = &entry.mutation;
            let Some(candidates) = by_file.get(target.file.as_path()) else {
                return MatchStatus::Missing;
            };
            if candidates
                .iter()
                .any(|(_, m)| m.file.fingerprint != target.source_fingerprint)
            {
                return MatchStatus::Stale;
            }
            let mut matches = candidates.iter().filter(|(_, m)| target.matches(m));
            match (matches.next(), matches.next()) {
                (None, _) => MatchStatus::Unmatched,
                (Some((id, _)), None) => MatchStatus::Matched(*id),
                _ => MatchStatus::Ambiguous,
            }
        })
        .collect();
    // A hand edit or merge can duplicate an identity under different IDs.
    // Treat every entry in that collision as ambiguous, never pick a winner.
    let mut counts = HashMap::new();
    for status in &statuses {
        if let MatchStatus::Matched(id) = status {
            *counts.entry(*id).or_insert(0) += 1;
        }
    }
    for status in &mut statuses {
        if let MatchStatus::Matched(id) = status
            && counts[id] > 1
        {
            *status = MatchStatus::Ambiguous;
        }
    }
    statuses
}

pub async fn check(config: &RunConfig, cancellation: &Cancellation) -> Result<()> {
    ensure!(
        config.lang != Lang::Vitest,
        "Vitest dismissal review is not supported yet"
    );
    let store = Store::load(&config.module_root)?;
    if store.dismissals.is_empty() {
        println!("No dismissals to check.");
        return Ok(());
    }
    let scope = Scope::new(&config.include, &config.exclude)?;
    let project = (config.lang == Lang::Rust)
        .then(|| crate::rust::Project::load(&config.module_root))
        .transpose()?;
    let mut excluded = vec![config.module_root.join(".mutie"), config.output_dir.clone()];
    if let Some(project) = &project {
        excluded.extend(project.excluded.clone());
    }
    let snapshot =
        crate::create_workspace(config.module_root.clone(), excluded, cancellation.clone()).await?;
    let store = Store::load(&snapshot.path())?;
    // Language tools may have side effects. Discover source identities from the
    // pristine snapshot, and run tools in a separate disposable copy.
    let discovery_workspace =
        crate::create_workspace(snapshot.path(), vec![], cancellation.clone()).await?;
    let limit = Duration::from_secs(config.baseline_timeout.get());
    let mutations = if project.is_some() {
        let project = crate::rust::Project::load(&snapshot.path())?;
        let output = crate::rust::discover(
            &discovery_workspace.path(),
            limit,
            config.max_log_bytes,
            config.max_capture_bytes,
            cancellation,
        )
        .await?;
        if output.interrupted {
            return Err(Interrupted.into());
        }
        project.validate_metadata(
            &discovery_workspace.path(),
            &crate::rust::metadata(&output)?,
        )?;
        drop(discovery_workspace);
        let discovery_cancellation = cancellation.clone();
        let source = snapshot.path();
        let found = tokio::task::spawn_blocking(move || {
            crate::rust::discovery::discover(
                &source,
                &project
                    .scope
                    .targets
                    .iter()
                    .map(|target| target.source.clone())
                    .collect::<Vec<_>>(),
                &scope,
                &discovery_cancellation,
            )
        })
        .await
        .context("Rust dismissal discovery panicked")??;
        for exclusion in found.exclusions {
            println!("Rust discovery excluded {exclusion}");
        }
        found.mutations
    } else {
        let output = crate::go::discover(
            &discovery_workspace.path(),
            crate::cli::TestScope::Full,
            limit,
            config.max_log_bytes,
            config.max_capture_bytes,
            cancellation,
        )
        .await?;
        if output.interrupted {
            return Err(Interrupted.into());
        }
        let mut files = crate::go::files(&discovery_workspace.path(), &output)?;
        drop(discovery_workspace);
        let discovery_cancellation = cancellation.clone();
        tokio::task::spawn_blocking(move || {
            mutation::discover_cancellable(
                &snapshot.path(),
                &mut files,
                &scope,
                &discovery_cancellation,
            )
        })
        .await
        .context("dismissal discovery panicked")??
    };
    cancellation.check()?;
    let statuses = match_dismissals(&store, config.lang, &mutations);
    let mut matched = 0;
    for (entry, status) in store.dismissals.iter().zip(statuses) {
        cancellation.check()?;
        let detail = match status {
            MatchStatus::Matched(id) => {
                matched += 1;
                format!("matched (current mutation {id})")
            }
            MatchStatus::Missing => {
                "missing (file removed, excluded, inactive, or has no mutations)".to_owned()
            }
            MatchStatus::Stale => "stale (source fingerprint changed)".to_owned(),
            MatchStatus::Unmatched => {
                "unmatched (source identity no longer identifies a mutation)".to_owned()
            }
            MatchStatus::Ambiguous => {
                "ambiguous (multiple mutations or dismissals share this identity)".to_owned()
            }
            MatchStatus::LanguageMismatch => {
                "language mismatch (decision belongs to another project language)".to_owned()
            }
        };
        println!(
            "Dismissal {}: {detail} — {:?}:{}:{}",
            entry.id, entry.mutation.file, entry.mutation.line, entry.mutation.column
        );
    }
    let review = store.dismissals.len() - matched;
    println!("{matched} matched; {review} require review.");
    ensure!(
        review == 0,
        "dismissals require review; inspect with dismiss list, then remove/re-add against a fresh report as needed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report_reader::SavedMutation;

    #[test]
    fn matching_uses_source_identity_even_when_current_ids_change_and_rejects_ambiguity()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        fs::write(
            directory.path().join("a.go"),
            "package p\nvar a, b = true, true\n",
        )?;
        let mutations = mutation::discover(
            directory.path(),
            vec!["a.go".into()],
            &Scope::new(&[], &[])?,
        )?;
        let m = &mutations[1];
        let saved = SavedMutation {
            id: 99,
            file: m.file.path.clone(),
            operator: m.operator,
            source_fingerprint: m.file.fingerprint.clone(),
            byte_start: m.range.start,
            byte_end: m.range.end,
            line: m.line,
            column: m.column,
            end_line: m.end_line,
            end_column: m.end_column,
            original: m.original().into(),
            replacement: m.replacement.to_string(),
            diff: None,
        };
        let mut store = Store::default();
        store.add(
            Lang::Go,
            MutationIdentity::from(&saved),
            "reviewed".into(),
            None,
        )?;
        store.validate()?;
        assert_eq!(
            match_dismissals(&store, Lang::Go, &mutations),
            [MatchStatus::Matched(2)]
        );
        assert_eq!(
            match_dismissals(&store, Lang::Go, &mutations[1..]),
            [MatchStatus::Matched(1)]
        );
        assert_eq!(
            match_dismissals(&store, Lang::Go, &[m.clone(), m.clone()]),
            [MatchStatus::Ambiguous]
        );
        assert_eq!(
            match_dismissals(&store, Lang::Go, &mutations[..1]),
            [MatchStatus::Unmatched]
        );
        assert_eq!(
            match_dismissals(&store, Lang::Rust, &mutations),
            [MatchStatus::LanguageMismatch]
        );
        let mut legacy = serde_json::to_value(&store.dismissals[0])?;
        legacy.as_object_mut().unwrap().remove("language");
        let legacy: Dismissal = serde_json::from_value(legacy)?;
        assert_eq!(legacy.language, Lang::Go);
        let mut unresolved = store.snapshot(Lang::Rust, &mutations, &Cancellation::default())?;
        unresolved.validate(Lang::Rust, &BTreeMap::new())?;
        assert!(unresolved.validate(Lang::Go, &BTreeMap::new()).is_err());
        store.add(
            Lang::Rust,
            MutationIdentity::from(&saved),
            "other language".into(),
            None,
        )?;
        let mut mixed = store.snapshot(Lang::Go, &mutations, &Cancellation::default())?;
        let mut saved = saved;
        saved.id = 2;
        mixed.validate(Lang::Go, &BTreeMap::from([(2, saved)]))?;
        assert_eq!(mixed.matched.len(), 1);
        assert_eq!(mixed.needs_review[0].status, MatchStatus::LanguageMismatch);
        Ok(())
    }
}
