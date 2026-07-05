use camino::Utf8PathBuf;

use crate::config::Config;
use crate::rule::{Evidence, Rule, Target};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanReport {
    pub roots: Vec<Utf8PathBuf>,
    pub matches: Vec<DependencyMatch>,
    pub skipped: Vec<SkippedPath>,
    pub failures: Vec<ScanFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyMatch {
    pub path: Utf8PathBuf,
    pub rule_id: &'static str,
    pub target: Target,
    pub evidence: Vec<MatchedEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchedEvidence {
    pub evidence: Evidence,
    pub path: Utf8PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedPath {
    pub path: Utf8PathBuf,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    ConfiguredSkipPath,
    Symlink,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanFailure {
    pub path: Option<Utf8PathBuf>,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("at least one scan root is required")]
    MissingRoot,
}

pub fn scan(config: &Config) -> Result<ScanReport, ScanError> {
    if config.roots.is_empty() {
        return Err(ScanError::MissingRoot);
    }

    Ok(ScanReport {
        roots: config.roots.clone(),
        ..ScanReport::default()
    })
}

pub fn rules(config: &Config) -> &[Rule] {
    config.rules
}
