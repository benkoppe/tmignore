use camino::Utf8PathBuf;

use crate::backend::{BackendDiagnostic, ExclusionChange, ExclusionStatus, TimeMachineBackend};
use crate::rule::Target;
use crate::scan::{MatchedEvidence, ScanReport};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub scan: ScanReport,
    pub actions: Vec<ExclusionAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExclusionAction {
    pub path: Utf8PathBuf,
    pub rule_id: &'static str,
    pub target: Target,
    pub evidence: Vec<MatchedEvidence>,
    pub outcome: ExclusionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExclusionOutcome {
    DryRun,
    AlreadyExcluded,
    NewlyExcluded,
    StatusFailed(BackendDiagnostic),
    AddFailed(BackendDiagnostic),
}

impl RunReport {
    pub fn dry_run(scan: ScanReport) -> Self {
        let actions = scan
            .matches
            .iter()
            .map(|dependency_match| ExclusionAction {
                path: dependency_match.path.clone(),
                rule_id: dependency_match.rule_id,
                target: dependency_match.target,
                evidence: dependency_match.evidence.clone(),
                outcome: ExclusionOutcome::DryRun,
            })
            .collect();

        Self { scan, actions }
    }

    pub fn apply(scan: ScanReport, backend: &impl TimeMachineBackend) -> Self {
        let actions = scan
            .matches
            .iter()
            .map(|dependency_match| {
                let outcome = match backend.exclusion_status(&dependency_match.path) {
                    ExclusionStatus::Excluded => ExclusionOutcome::AlreadyExcluded,
                    ExclusionStatus::Included => {
                        match backend.add_exclusion(&dependency_match.path) {
                            Ok(ExclusionChange::AlreadyExcluded) => {
                                ExclusionOutcome::AlreadyExcluded
                            }
                            Ok(ExclusionChange::NewlyExcluded) => ExclusionOutcome::NewlyExcluded,
                            Ok(ExclusionChange::DryRun) => ExclusionOutcome::DryRun,
                            Err(diagnostic) => ExclusionOutcome::AddFailed(diagnostic),
                        }
                    }
                    ExclusionStatus::Unknown(diagnostic) => {
                        ExclusionOutcome::StatusFailed(diagnostic)
                    }
                };

                ExclusionAction {
                    path: dependency_match.path.clone(),
                    rule_id: dependency_match.rule_id,
                    target: dependency_match.target,
                    evidence: dependency_match.evidence.clone(),
                    outcome,
                }
            })
            .collect();

        Self { scan, actions }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::{HashMap, HashSet};

    use camino::{Utf8Path, Utf8PathBuf};

    use super::*;
    use crate::rule::{Evidence, EvidenceKind, TargetKind};
    use crate::scan::{DependencyMatch, MatchedEvidence};

    #[test]
    fn dry_run_creates_actions_without_backend() {
        let scan = scan_report(vec![dependency_match("/tmp/project/node_modules", "node")]);

        let report = RunReport::dry_run(scan);

        assert_eq!(report.actions.len(), 1);
        assert_eq!(report.actions[0].outcome, ExclusionOutcome::DryRun);
    }

    #[test]
    fn apply_excludes_included_paths() {
        let scan = scan_report(vec![dependency_match("/tmp/project/node_modules", "node")]);
        let backend = FakeBackend::default();

        let report = RunReport::apply(scan, &backend);

        assert_eq!(report.actions[0].outcome, ExclusionOutcome::NewlyExcluded);
        assert_eq!(
            backend.added_paths(),
            vec![Utf8PathBuf::from("/tmp/project/node_modules")]
        );
    }

    #[test]
    fn apply_skips_already_excluded_paths() {
        let scan = scan_report(vec![dependency_match("/tmp/project/target", "rust")]);
        let backend = FakeBackend::default().with_excluded("/tmp/project/target");

        let report = RunReport::apply(scan, &backend);

        assert_eq!(report.actions[0].outcome, ExclusionOutcome::AlreadyExcluded);
        assert!(backend.added_paths().is_empty());
    }

    #[test]
    fn apply_reports_status_failures() {
        let scan = scan_report(vec![dependency_match("/tmp/project/target", "rust")]);
        let diagnostic = diagnostic("/tmp/project/target", "status failed");
        let backend = FakeBackend::default().with_status_failure(diagnostic.clone());

        let report = RunReport::apply(scan, &backend);

        assert_eq!(
            report.actions[0].outcome,
            ExclusionOutcome::StatusFailed(diagnostic)
        );
        assert!(backend.added_paths().is_empty());
    }

    #[test]
    fn apply_reports_add_failures_and_continues() {
        let scan = scan_report(vec![
            dependency_match("/tmp/project/target", "rust"),
            dependency_match("/tmp/project/node_modules", "node"),
        ]);
        let diagnostic = diagnostic("/tmp/project/target", "add failed");
        let backend = FakeBackend::default().with_add_failure(diagnostic.clone());

        let report = RunReport::apply(scan, &backend);

        assert_eq!(
            report.actions[0].outcome,
            ExclusionOutcome::AddFailed(diagnostic)
        );
        assert_eq!(report.actions[1].outcome, ExclusionOutcome::NewlyExcluded);
        assert_eq!(
            backend.added_paths(),
            vec![
                Utf8PathBuf::from("/tmp/project/target"),
                Utf8PathBuf::from("/tmp/project/node_modules")
            ]
        );
    }

    fn scan_report(matches: Vec<DependencyMatch>) -> ScanReport {
        ScanReport {
            matches,
            ..ScanReport::default()
        }
    }

    fn dependency_match(path: &'static str, rule_id: &'static str) -> DependencyMatch {
        DependencyMatch {
            path: Utf8PathBuf::from(path),
            rule_id,
            target: Target {
                path: "target",
                kind: TargetKind::Directory,
            },
            evidence: vec![MatchedEvidence {
                evidence: Evidence::candidate_parent("Cargo.toml", EvidenceKind::File),
                path: Utf8PathBuf::from("/tmp/project/Cargo.toml"),
            }],
        }
    }

    fn diagnostic(path: &'static str, message: &'static str) -> BackendDiagnostic {
        BackendDiagnostic {
            path: Utf8PathBuf::from(path),
            message: message.to_string(),
            stdout: String::new(),
            stderr: String::new(),
            status_code: Some(1),
        }
    }

    #[derive(Default)]
    struct FakeBackend {
        excluded: HashSet<Utf8PathBuf>,
        status_failures: HashMap<Utf8PathBuf, BackendDiagnostic>,
        add_failures: HashMap<Utf8PathBuf, BackendDiagnostic>,
        added: RefCell<Vec<Utf8PathBuf>>,
    }

    impl FakeBackend {
        fn with_excluded(mut self, path: &'static str) -> Self {
            self.excluded.insert(Utf8PathBuf::from(path));
            self
        }

        fn with_status_failure(mut self, diagnostic: BackendDiagnostic) -> Self {
            self.status_failures
                .insert(diagnostic.path.clone(), diagnostic);
            self
        }

        fn with_add_failure(mut self, diagnostic: BackendDiagnostic) -> Self {
            self.add_failures
                .insert(diagnostic.path.clone(), diagnostic);
            self
        }

        fn added_paths(&self) -> Vec<Utf8PathBuf> {
            self.added.borrow().clone()
        }
    }

    impl TimeMachineBackend for FakeBackend {
        fn exclusion_status(&self, path: &Utf8Path) -> ExclusionStatus {
            if let Some(diagnostic) = self.status_failures.get(path) {
                return ExclusionStatus::Unknown(diagnostic.clone());
            }

            if self.excluded.contains(path) {
                ExclusionStatus::Excluded
            } else {
                ExclusionStatus::Included
            }
        }

        fn add_exclusion(&self, path: &Utf8Path) -> Result<ExclusionChange, BackendDiagnostic> {
            self.added.borrow_mut().push(path.to_path_buf());

            if let Some(diagnostic) = self.add_failures.get(path) {
                Err(diagnostic.clone())
            } else {
                Ok(ExclusionChange::NewlyExcluded)
            }
        }
    }
}
