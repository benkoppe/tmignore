use std::io;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};
use walkdir::WalkDir;

use crate::config::Config;
use crate::rule::{Evidence, EvidenceKind, Rule, RuleCase, Target, TargetKind};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

    let mut report = ScanReport {
        roots: config.roots.clone(),
        ..ScanReport::default()
    };

    for root in &config.roots {
        scan_root(root, config, &mut report);
    }

    Ok(report)
}

pub fn rules(config: &Config) -> &[Rule] {
    config.rules
}

fn scan_root(root: &Utf8Path, config: &Config, report: &mut ScanReport) {
    match fs_err::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            report.skipped.push(SkippedPath {
                path: root.to_path_buf(),
                reason: SkipReason::Symlink,
            });
            return;
        }
        Ok(_) => {}
        Err(error) => {
            report.failures.push(ScanFailure {
                path: Some(root.to_path_buf()),
                message: error.to_string(),
            });
            return;
        }
    }

    let mut entries = WalkDir::new(root).follow_links(false).into_iter();

    while let Some(entry_result) = entries.next() {
        let entry = match entry_result {
            Ok(entry) => entry,
            Err(error) => {
                report.failures.push(ScanFailure {
                    path: error.path().and_then(path_to_utf8),
                    message: error.to_string(),
                });
                continue;
            }
        };

        let Some(path) = path_to_utf8(entry.path()) else {
            report.failures.push(ScanFailure {
                path: None,
                message: format!("path is not valid UTF-8: {}", entry.path().display()),
            });

            if entry.file_type().is_dir() {
                entries.skip_current_dir();
            }

            continue;
        };

        if should_skip_path(&path, &config.skip_paths) {
            let is_dir = entry.file_type().is_dir();
            report.skipped.push(SkippedPath {
                path,
                reason: SkipReason::ConfiguredSkipPath,
            });

            if is_dir {
                entries.skip_current_dir();
            }

            continue;
        }

        if entry.file_type().is_symlink() {
            report.skipped.push(SkippedPath {
                path,
                reason: SkipReason::Symlink,
            });
            continue;
        }

        if !entry.file_type().is_dir() {
            continue;
        }

        let (dependency_match, failures) = match_dependency(&path, config.rules);
        report.failures.extend(failures);

        if let Some(dependency_match) = dependency_match {
            report.matches.push(dependency_match);
            entries.skip_current_dir();
        }
    }
}

fn match_dependency(
    candidate_path: &Utf8Path,
    rules: &[Rule],
) -> (Option<DependencyMatch>, Vec<ScanFailure>) {
    let mut failures = Vec::new();

    for rule in rules {
        for rule_case in rule.cases {
            let Some(target) = matching_target(candidate_path, rule_case) else {
                continue;
            };

            let mut matched_evidence = Vec::new();
            let mut case_failures = Vec::new();
            let mut requirements_satisfied = true;

            for requirement in rule_case.requirements {
                match satisfy_requirement(candidate_path, requirement.any_of) {
                    RequirementResult::Satisfied(evidence) => matched_evidence.push(evidence),
                    RequirementResult::Unsatisfied { failures } => {
                        requirements_satisfied = false;
                        case_failures.extend(failures);
                        break;
                    }
                }
            }

            if requirements_satisfied {
                return (
                    Some(DependencyMatch {
                        path: candidate_path.to_path_buf(),
                        rule_id: rule.id,
                        target,
                        evidence: matched_evidence,
                    }),
                    failures,
                );
            }

            failures.extend(case_failures);
        }
    }

    (None, failures)
}

fn matching_target(candidate_path: &Utf8Path, rule_case: &RuleCase) -> Option<Target> {
    rule_case.targets.iter().copied().find(|target| {
        target.kind == TargetKind::Directory && target_matches(candidate_path, *target)
    })
}

fn target_matches(candidate_path: &Utf8Path, target: Target) -> bool {
    if target.path.contains('/') {
        return candidate_path.ends_with(target.path);
    }

    candidate_path.file_name() == Some(target.path)
}

enum RequirementResult {
    Satisfied(MatchedEvidence),
    Unsatisfied { failures: Vec<ScanFailure> },
}

fn satisfy_requirement(
    candidate_path: &Utf8Path,
    evidence_options: &[Evidence],
) -> RequirementResult {
    let mut failures = Vec::new();

    for evidence in evidence_options {
        let Some(path) = evidence.resolve_against(candidate_path) else {
            failures.push(ScanFailure {
                path: Some(candidate_path.to_path_buf()),
                message: format!("cannot resolve evidence path `{}`", evidence.path),
            });
            continue;
        };

        match evidence_exists(&path, evidence.kind) {
            Ok(true) => {
                return RequirementResult::Satisfied(MatchedEvidence {
                    evidence: *evidence,
                    path,
                });
            }
            Ok(false) => {}
            Err(error) => failures.push(ScanFailure {
                path: Some(path),
                message: error.to_string(),
            }),
        }
    }

    RequirementResult::Unsatisfied { failures }
}

fn evidence_exists(path: &Utf8Path, kind: EvidenceKind) -> io::Result<bool> {
    match fs_err::symlink_metadata(path) {
        Ok(metadata) => {
            let file_type = metadata.file_type();
            Ok(match kind {
                EvidenceKind::File => file_type.is_file(),
                EvidenceKind::Directory => file_type.is_dir(),
                EvidenceKind::Any => true,
            })
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn should_skip_path(path: &Utf8Path, skip_paths: &[Utf8PathBuf]) -> bool {
    skip_paths
        .iter()
        .any(|skip_path| path.starts_with(skip_path))
}

fn path_to_utf8(path: &Path) -> Option<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path.to_path_buf()).ok()
}

#[cfg(test)]
mod tests {
    use std::fs::Permissions;
    use std::os::unix::fs::{self as unix_fs, PermissionsExt};

    use tempfile::TempDir;

    use super::*;
    use crate::rule::{EvidenceBase, Requirement};

    const STRICT_RULES: &[Rule] = &[Rule {
        id: "strict",
        cases: &[RuleCase {
            targets: &[Target {
                path: ".strict-cache",
                kind: TargetKind::Directory,
            }],
            requirements: &[
                Requirement {
                    any_of: &[Evidence {
                        path: "strict.toml",
                        kind: EvidenceKind::File,
                        base: EvidenceBase::CandidateParent,
                    }],
                },
                Requirement {
                    any_of: &[Evidence {
                        path: "strict.lock",
                        kind: EvidenceKind::File,
                        base: EvidenceBase::CandidateParent,
                    }],
                },
            ],
        }],
    }];

    #[test]
    fn matches_node_modules_when_package_json_exists() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");

        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].rule_id, "node");
        assert_eq!(report.matches[0].target.path, "node_modules");
        assert_eq!(report.matches[0].evidence[0].evidence.path, "package.json");
    }

    #[test]
    fn does_not_match_node_modules_without_package_json() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules");

        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);

        assert!(report.matches.is_empty());
        assert!(report.failures.is_empty());
    }

    #[test]
    fn matches_one_of_multiple_targets() {
        let fixture = Fixture::new();
        fixture.dir("project/venv");
        fixture.file("project/pyproject.toml");

        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].rule_id, "python-venv");
        assert_eq!(report.matches[0].target.path, "venv");
    }

    #[test]
    fn requires_all_requirements() {
        let fixture = Fixture::new();
        fixture.dir("project/.strict-cache");
        fixture.file("project/strict.toml");

        let report = scan_fixture(&fixture, STRICT_RULES, &[]);
        assert!(report.matches.is_empty());

        fixture.file("project/strict.lock");

        let report = scan_fixture(&fixture, STRICT_RULES, &[]);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].evidence.len(), 2);
    }

    #[test]
    fn allows_any_evidence_within_requirement() {
        let fixture = Fixture::new();
        fixture.dir("project/vendor");
        fixture.file("project/Gemfile");

        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].rule_id, "vendor");
        assert_eq!(report.matches[0].evidence[0].evidence.path, "Gemfile");
    }

    #[test]
    fn finds_multiple_matches() {
        let fixture = Fixture::new();
        fixture.dir("first/node_modules");
        fixture.file("first/package.json");
        fixture.dir("second/target");
        fixture.file("second/Cargo.toml");

        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);

        assert_eq!(report.matches.len(), 2);
    }

    #[test]
    fn prunes_matched_directories() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules/nested/node_modules");
        fixture.file("project/package.json");
        fixture.file("project/node_modules/nested/package.json");

        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].path, fixture.path("project/node_modules"));
    }

    #[test]
    fn skips_configured_paths() {
        let fixture = Fixture::new();
        fixture.dir("ignored/project/node_modules");
        fixture.file("ignored/project/package.json");

        let skip_path = fixture.path("ignored");
        let report = scan_fixture(
            &fixture,
            crate::rule::DEFAULT_RULES,
            std::slice::from_ref(&skip_path),
        );

        assert!(report.matches.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].path, skip_path);
        assert_eq!(report.skipped[0].reason, SkipReason::ConfiguredSkipPath);
    }

    #[test]
    fn does_not_follow_symlinks() {
        let fixture = Fixture::new();
        fixture.dir("real/project/node_modules");
        fixture.file("real/project/package.json");
        unix_fs::symlink(fixture.path("real"), fixture.path("linked")).unwrap();

        let config = Config {
            roots: vec![fixture.path("linked")],
            skip_paths: Vec::new(),
            dry_run: true,
            rules: crate::rule::DEFAULT_RULES,
        };
        let report = scan(&config).unwrap();

        assert!(report.matches.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason, SkipReason::Symlink);
    }

    #[test]
    fn symlinks_do_not_prune_later_siblings() {
        let fixture = Fixture::new();
        fixture.dir("real");
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");
        unix_fs::symlink(fixture.path("real"), fixture.path("linked")).unwrap();

        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].path, fixture.path("project/node_modules"));
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason, SkipReason::Symlink);
    }

    #[test]
    fn handles_paths_with_spaces() {
        let fixture = Fixture::new();
        fixture.dir("project with spaces/node_modules");
        fixture.file("project with spaces/package.json");

        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(
            report.matches[0].path,
            fixture.path("project with spaces/node_modules")
        );
    }

    #[test]
    fn records_unreadable_directory_failures_without_aborting() {
        let fixture = Fixture::new();
        fixture.dir("blocked/child");
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");
        let blocked_path = fixture.path("blocked");

        fs_err::set_permissions(&blocked_path, Permissions::from_mode(0o000)).unwrap();
        let report = scan_fixture(&fixture, crate::rule::DEFAULT_RULES, &[]);
        fs_err::set_permissions(&blocked_path, Permissions::from_mode(0o700)).unwrap();

        assert_eq!(report.matches.len(), 1);
        assert!(report.failures.iter().any(|failure| {
            failure.path.as_ref() == Some(&blocked_path)
                && failure.message.contains("Permission denied")
        }));
    }

    #[test]
    fn matches_relative_candidate_paths() {
        let target = Target {
            path: "target",
            kind: TargetKind::Directory,
        };

        assert!(target_matches(Utf8Path::new("./target"), target));
        assert!(target_matches(Utf8Path::new("target"), target));
    }

    #[test]
    fn resolves_evidence_for_relative_single_component_candidates() {
        let evidence = Evidence::candidate_parent("Cargo.toml", EvidenceKind::File);

        assert_eq!(
            evidence.resolve_against(Utf8Path::new("target")),
            Some(Utf8PathBuf::from("Cargo.toml"))
        );
    }

    fn scan_fixture(
        fixture: &Fixture,
        rules: &'static [Rule],
        skip_paths: &[Utf8PathBuf],
    ) -> ScanReport {
        let config = Config {
            roots: vec![fixture.root()],
            skip_paths: skip_paths.to_vec(),
            dry_run: true,
            rules,
        };

        scan(&config).unwrap()
    }

    struct Fixture {
        temp_dir: TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                temp_dir: tempfile::tempdir().unwrap(),
            }
        }

        fn root(&self) -> Utf8PathBuf {
            Utf8PathBuf::from_path_buf(self.temp_dir.path().to_path_buf()).unwrap()
        }

        fn path(&self, path: &str) -> Utf8PathBuf {
            self.root().join(path)
        }

        fn dir(&self, path: &str) {
            fs_err::create_dir_all(self.path(path)).unwrap();
        }

        fn file(&self, path: &str) {
            let path = self.path(path);
            fs_err::create_dir_all(path.parent().unwrap()).unwrap();
            fs_err::write(path, b"").unwrap();
        }
    }
}
