use camino::Utf8PathBuf;

pub mod tmutil;

pub use tmutil::{CommandOutput, CommandRunner, ProcessCommandRunner, TmutilBackend};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExclusionStatus {
    Included,
    Excluded,
    Unknown(BackendDiagnostic),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExclusionChange {
    AlreadyExcluded,
    NewlyExcluded,
    DryRun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDiagnostic {
    pub path: Utf8PathBuf,
    pub message: String,
    pub stdout: String,
    pub stderr: String,
    pub status_code: Option<i32>,
}

pub trait TimeMachineBackend {
    fn exclusion_status(&self, path: &camino::Utf8Path) -> ExclusionStatus;

    fn add_exclusion(&self, path: &camino::Utf8Path) -> Result<ExclusionChange, BackendDiagnostic>;
}
