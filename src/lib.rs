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
pub use config::{BuiltinRuleMode, Config, ConfigError, PreparedConfig, RunMode};
pub use rule::{
    Evidence, EvidenceBase, EvidenceKind, MatchedRule, MatchedRuleEvidence, Requirement, Rule,
    RuleCase, Target, TargetKind, default_rules,
};
pub use run::{ExclusionAction, ExclusionOutcome, RunReport};
pub use scan::{DependencyMatch, MatchedEvidence, ScanFailure, ScanReport, SkippedPath};
