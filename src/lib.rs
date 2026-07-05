pub mod backend;
pub mod cli;
pub mod config;
pub mod report;
pub mod rule;
pub mod run;
pub mod scan;

pub use backend::{
    BackendDiagnostic, CommandOutput, CommandRunner, ExclusionChange, ExclusionStatus,
    ProcessCommandRunner, TimeMachineBackend, TmutilBackend,
};
pub use config::{Config, ConfigError, PreparedConfig, RunMode};
pub use rule::{
    DEFAULT_RULES, Evidence, EvidenceBase, EvidenceKind, MatchedRule, MatchedRuleEvidence,
    Requirement, Rule, RuleCase, Target, TargetKind,
};
pub use run::{ExclusionAction, ExclusionOutcome, RunReport};
pub use scan::{DependencyMatch, MatchedEvidence, ScanFailure, ScanReport, SkippedPath};
