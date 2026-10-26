//! Account observations and project-count language shares, independent of collection.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

/// GitHub observations supplied by the caller, readable and writable through Serde.
///
/// The caller supplies the complete collection of owned and GitHub-recognized
/// contributed repositories. Parsing verifies the format, not collection completeness.
/// All fields are required; unknown fields and unsupported versions are rejected.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AccountSnapshot {
    pub schema_version: SnapshotVersion,
    pub account_id: u64,
    pub repositories: Vec<RepositoryObservation>,
}

/// Supported snapshot formats, independent of the program's release version.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum SnapshotVersion {
    V1 = 1,
}

/// Current repository metadata identified by its stable GitHub numeric ID.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryObservation {
    pub repository_id: u64,
    pub visibility: RepositoryVisibility,
    pub is_fork: bool,
    /// GitHub's language name unchanged; null means a known absence.
    /// Omitting the field is an error because absence of data is not an observation.
    #[serde(deserialize_with = "Option::deserialize")]
    pub primary_language: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RepositoryVisibility {
    Public,
    Private,
    Internal,
}

/// Calculate fractions of eligible projects, keyed by unchanged language names.
///
/// Each distinct public, non-fork repository with a known primary language has
/// one vote. Identical observations count once; conflicting observations for the
/// same ID fail rather than let input order determine the answer. No additional
/// contribution or age rules apply. The input is borrowed without mutation.
///
/// Shares are ordered by language name and range from zero to one. An empty
/// eligible collection returns an empty map. Collection and file access belong
/// to the caller; this operation performs no I/O.
pub fn calculate_language_shares(
    account_snapshot: &AccountSnapshot,
) -> Result<BTreeMap<String, f64>, ConflictingRepository> {
    let mut distinct_repositories = BTreeMap::new();
    for repository_observation in &account_snapshot.repositories {
        if let Some(previous_observation) = distinct_repositories
            .insert(repository_observation.repository_id, repository_observation)
            && previous_observation != repository_observation
        {
            return Err(ConflictingRepository {
                repository_id: repository_observation.repository_id,
            });
        }
    }

    let mut language_repository_counts = BTreeMap::new();
    for repository_observation in distinct_repositories.values() {
        if repository_observation.visibility != RepositoryVisibility::Public
            || repository_observation.is_fork
        {
            continue;
        }
        if let Some(language_name) = &repository_observation.primary_language {
            *language_repository_counts
                .entry(language_name.clone())
                .or_insert(0_usize) += 1;
        }
    }

    let eligible_repository_count: usize = language_repository_counts.values().sum();
    Ok(language_repository_counts
        .into_iter()
        .map(|(language_name, repository_count)| {
            (
                language_name,
                repository_count as f64 / eligible_repository_count as f64,
            )
        })
        .collect())
}

/// Two observations disagree about the same repository; no shares were produced.
#[derive(Debug, Eq, PartialEq)]
pub struct ConflictingRepository {
    pub repository_id: u64,
}

impl fmt::Display for ConflictingRepository {
    fn fmt(&self, output_formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output_formatter,
            "conflicting observations for repository {}; collect a consistent snapshot",
            self.repository_id
        )
    }
}

impl Error for ConflictingRepository {}

impl TryFrom<u32> for SnapshotVersion {
    type Error = String;

    fn try_from(schema_version: u32) -> Result<Self, Self::Error> {
        if schema_version == Self::V1 as u32 {
            Ok(Self::V1)
        } else {
            Err(format!(
                "unsupported snapshot schema version {schema_version}; use a reader supporting that version"
            ))
        }
    }
}

impl From<SnapshotVersion> for u32 {
    fn from(schema_version: SnapshotVersion) -> Self {
        schema_version as Self
    }
}
