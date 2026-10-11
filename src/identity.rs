//! Source identities shared by saved-run selection and persistent dismissals.
use std::path::{Component, PathBuf};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    mutation::{Mutation, Operator},
    report_reader::SavedMutation,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MutationIdentity {
    pub file: PathBuf,
    pub operator: Operator,
    pub source_fingerprint: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub original: String,
    pub replacement: String,
}

impl From<&SavedMutation> for MutationIdentity {
    fn from(mutation: &SavedMutation) -> Self {
        Self {
            file: mutation.file.clone(),
            operator: mutation.operator,
            source_fingerprint: mutation.source_fingerprint.clone(),
            byte_start: mutation.byte_start,
            byte_end: mutation.byte_end,
            line: mutation.line,
            column: mutation.column,
            end_line: mutation.end_line,
            end_column: mutation.end_column,
            original: mutation.original.clone(),
            replacement: mutation.replacement.clone(),
        }
    }
}

impl From<&Mutation> for MutationIdentity {
    fn from(m: &Mutation) -> Self {
        Self {
            file: m.file.path.clone(),
            operator: m.operator,
            source_fingerprint: m.file.fingerprint.clone(),
            byte_start: m.range.start,
            byte_end: m.range.end,
            line: m.line,
            column: m.column,
            end_line: m.end_line,
            end_column: m.end_column,
            original: m.original().to_owned(),
            replacement: m.replacement.to_string(),
        }
    }
}

impl MutationIdentity {
    pub fn matches(&self, current: &Mutation) -> bool {
        self.file == current.file.path
            && self.source_fingerprint == current.file.fingerprint
            && self.operator == current.operator
            && self.byte_start == current.range.start
            && self.byte_end == current.range.end
            && self.original == current.original()
            && self.replacement == current.replacement
            && self.line == current.line
            && self.column == current.column
            && self.end_line == current.end_line
            && self.end_column == current.end_column
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.file.as_os_str().is_empty()
                && self
                    .file
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "invalid module-relative source path"
        );
        ensure!(
            self.source_fingerprint
                .strip_prefix("sha256:")
                .is_some_and(|hash| hash.len() == 64
                    && hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))),
            "invalid source fingerprint"
        );
        ensure!(
            self.byte_start < self.byte_end
                && self.byte_end - self.byte_start == self.original.len()
                && self.line > 0
                && self.column > 0
                && self.end_line >= self.line
                && self.end_column > 0
                && (self.end_line > self.line || self.end_column > self.column),
            "invalid source coordinates"
        );
        ensure!(
            self.original != self.replacement,
            "replacement must change the source"
        );
        Ok(())
    }
}
