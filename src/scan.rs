use std::path::Path;
use std::{env, io};

use camino::{Utf8Path, Utf8PathBuf};
use walkdir::WalkDir;

use crate::config::PreparedConfig;
use crate::rule::{Evidence, EvidenceBase, EvidenceKind, Rule, RuleCase, Target, TargetKind};

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
    pub rule_id: String,
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

pub fn scan(config: &PreparedConfig) -> Result<ScanReport, ScanError> {
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

fn scan_root(root: &Utf8Path, config: &PreparedConfig, report: &mut ScanReport) {
    match scan_root_symlink_component(root) {
        Ok(Some(path)) => {
            report.skipped.push(SkippedPath {
                path,
                reason: SkipReason::Symlink,
            });
            return;
        }
        Ok(None) => {}
        Err(error) => {
            report.failures.push(ScanFailure {
                path: Some(root.to_path_buf()),
                message: error.to_string(),
            });
            return;
        }
    }

    match fs_err::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            report.skipped.push(SkippedPath {
                path: root.to_path_buf(),
                reason: SkipReason::Symlink,
            });
            return;
        }
        Ok(metadata) if metadata.file_type().is_dir() => {}
        Ok(_) => {
            report.failures.push(ScanFailure {
                path: Some(root.to_path_buf()),
                message: "scan root is not a directory".to_string(),
            });
            return;
        }
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

        let (dependency_match, failures) = match_dependency(&path, &config.rules);
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
        for rule_case in &rule.cases {
            let Some(target) = matching_target(candidate_path, rule_case) else {
                continue;
            };

            let mut matched_evidence = Vec::new();
            let mut case_failures = Vec::new();
            let mut requirements_satisfied = true;

            for requirement in &rule_case.requirements {
                match satisfy_requirement(candidate_path, &target, &requirement.any_of) {
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
                        rule_id: rule.id.clone(),
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
    rule_case
        .targets
        .iter()
        .find(|target| {
            target.kind == TargetKind::Directory && target_matches(candidate_path, target)
        })
        .cloned()
}

fn target_matches(candidate_path: &Utf8Path, target: &Target) -> bool {
    if target.path.contains('/') {
        return candidate_path.ends_with(&target.path);
    }

    candidate_path.file_name() == Some(target.path.as_str())
}

enum RequirementResult {
    Satisfied(MatchedEvidence),
    Unsatisfied { failures: Vec<ScanFailure> },
}

fn satisfy_requirement(
    candidate_path: &Utf8Path,
    target: &Target,
    evidence_options: &[Evidence],
) -> RequirementResult {
    let mut failures = Vec::new();

    for evidence in evidence_options {
        let path = evidence.resolve_against_target(candidate_path, &target.path);

        let symlink_base = evidence_symlink_base(candidate_path, &target.path, evidence);

        match evidence_exists(&path, evidence.kind, symlink_base) {
            Ok(true) => {
                return RequirementResult::Satisfied(MatchedEvidence {
                    evidence: evidence.clone(),
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

fn evidence_exists(
    path: &Utf8Path,
    kind: EvidenceKind,
    symlink_base: &Utf8Path,
) -> io::Result<bool> {
    if let Some(symlink_path) = symlink_component_between(symlink_base, path, false)? {
        return Err(io::Error::other(format!(
            "path contains symlink: {symlink_path}"
        )));
    }

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

fn scan_root_symlink_component(root: &Utf8Path) -> io::Result<Option<Utf8PathBuf>> {
    let Some(home) = env::home_dir().and_then(|path| Utf8PathBuf::from_path_buf(path).ok()) else {
        return Ok(None);
    };

    if root.starts_with(&home) {
        symlink_component_between(&home, root, true)
    } else {
        Ok(None)
    }
}

fn symlink_component_between(
    base: &Utf8Path,
    path: &Utf8Path,
    include_base: bool,
) -> io::Result<Option<Utf8PathBuf>> {
    let Ok(relative_path) = path.strip_prefix(base) else {
        return Ok(None);
    };
    let mut current = base.to_path_buf();

    if include_base {
        match fs_err::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(Some(current)),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    }

    for component in relative_path.components() {
        match component.as_str() {
            "/" | "." => continue,
            component => current.push(component),
        }

        match fs_err::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(Some(current.clone())),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
    }

    Ok(None)
}

fn evidence_symlink_base<'a>(
    candidate_path: &'a Utf8Path,
    target_path: &str,
    evidence: &Evidence,
) -> &'a Utf8Path {
    match evidence.base {
        EvidenceBase::Candidate => candidate_path,
        EvidenceBase::CandidateParent => candidate_path
            .parent()
            .unwrap_or_else(|| Utf8Path::new(".")),
        EvidenceBase::TargetParent => target_parent(candidate_path, target_path),
    }
}

fn target_parent<'a>(candidate_path: &'a Utf8Path, target_path: &str) -> &'a Utf8Path {
    let mut parent = candidate_path;

    for _ in Utf8Path::new(target_path).components() {
        parent = parent.parent().unwrap_or_else(|| Utf8Path::new("."));
    }

    parent
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
    use crate::config::{Config, RunMode};
    use crate::rule::{EvidenceBase, Requirement};

    fn strict_rules() -> Vec<Rule> {
        vec![Rule::new(
            "strict",
            vec![RuleCase::new(
                vec![Target::directory(".strict-cache")],
                vec![
                    Requirement::any_of(vec![Evidence {
                        path: "strict.toml".to_string(),
                        kind: EvidenceKind::File,
                        base: EvidenceBase::CandidateParent,
                    }]),
                    Requirement::any_of(vec![Evidence {
                        path: "strict.lock".to_string(),
                        kind: EvidenceKind::File,
                        base: EvidenceBase::CandidateParent,
                    }]),
                ],
            )],
        )]
    }

    #[test]
    fn matches_node_modules_when_package_json_exists() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].rule_id, "node.node-modules");
        assert_eq!(report.matches[0].target.path, "node_modules");
        assert_eq!(report.matches[0].evidence[0].evidence.path, "package.json");
    }

    #[test]
    fn does_not_match_node_modules_without_package_json() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules");

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

        assert!(report.matches.is_empty());
        assert!(report.failures.is_empty());
    }

    #[test]
    fn matches_one_of_multiple_targets() {
        let fixture = Fixture::new();
        fixture.dir("project/venv");
        fixture.file("project/pyproject.toml");

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].rule_id, "python.venv");
        assert_eq!(report.matches[0].target.path, "venv");
    }

    #[test]
    fn requires_all_requirements() {
        let fixture = Fixture::new();
        fixture.dir("project/.strict-cache");
        fixture.file("project/strict.toml");

        let report = scan_fixture(&fixture, strict_rules(), &[]);
        assert!(report.matches.is_empty());

        fixture.file("project/strict.lock");

        let report = scan_fixture(&fixture, strict_rules(), &[]);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].evidence.len(), 2);
    }

    #[test]
    fn allows_any_evidence_within_requirement() {
        let fixture = Fixture::new();
        fixture.dir("project/.gradle");
        fixture.file("project/settings.gradle.kts");

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].rule_id, "gradle.cache");
        assert_eq!(
            report.matches[0].evidence[0].evidence.path,
            "settings.gradle.kts"
        );
    }

    #[test]
    fn default_rules_match_declared_dependency_directories() {
        let fixture = Fixture::new();
        let cases = [
            ("node.node-modules", "node_modules", "package.json"),
            ("node.parcel-cache", ".parcel-cache", "package.json"),
            ("rust.cargo-target", "target", "Cargo.toml"),
            ("php.composer-vendor", "vendor", "composer.json"),
            ("go.vendor", "vendor", "go.mod"),
            ("ruby.bundle-vendor", "vendor/bundle", "Gemfile"),
            ("python.venv", ".venv", "pyproject.toml"),
            ("python.tox", ".tox", "tox.ini"),
            ("python.nox", ".nox", "noxfile.py"),
            ("swift.build", ".build", "Package.swift"),
            ("elixir.deps", "deps", "mix.exs"),
            ("elixir.build", "_build", "mix.exs"),
            ("gradle.cache", ".gradle", "settings.gradle"),
            ("gradle.build", "build", "build.gradle.kts"),
            ("dart.tool", ".dart_tool", "pubspec.yaml"),
            ("dart.build", "build", "pubspec.yaml"),
            ("haskell.stack-work", ".stack-work", "stack.yaml"),
            ("vagrant.state", ".vagrant", "Vagrantfile"),
            ("ios.carthage", "Carthage", "Cartfile"),
            ("ios.cocoapods", "Pods", "Podfile"),
            ("terragrunt.cache", ".terragrunt-cache", "terragrunt.hcl"),
            ("aws-cdk.out", "cdk.out", "cdk.json"),
            ("java.maven-target", "target", "pom.xml"),
            ("scala.sbt-target", "target", "project/plugins.sbt"),
        ];

        for (index, (_, target, evidence)) in cases.iter().enumerate() {
            let project = format!("project-{index}");
            fixture.dir(&format!("{project}/{target}"));
            fixture.file(&format!("{project}/{evidence}"));
        }

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

        assert_eq!(report.matches.len(), cases.len());
        for (rule_id, target, evidence) in cases {
            assert!(report.matches.iter().any(|dependency_match| {
                dependency_match.rule_id == rule_id
                    && dependency_match.target.path == target
                    && dependency_match
                        .evidence
                        .iter()
                        .any(|matched_evidence| matched_evidence.evidence.path == evidence)
            }));
        }
    }

    #[test]
    fn broad_default_targets_do_not_match_without_evidence() {
        let fixture = Fixture::new();
        fixture.dir("project/build");
        fixture.dir("project/target");
        fixture.dir("project/vendor");
        fixture.dir("project/.build");
        fixture.dir("project/_build");

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

        assert!(report.matches.is_empty());
        assert!(report.failures.is_empty());
    }

    #[test]
    fn finds_multiple_matches() {
        let fixture = Fixture::new();
        fixture.dir("first/node_modules");
        fixture.file("first/package.json");
        fixture.dir("second/target");
        fixture.file("second/Cargo.toml");

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

        assert_eq!(report.matches.len(), 2);
    }

    #[test]
    fn prunes_matched_directories() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules/nested/node_modules");
        fixture.file("project/package.json");
        fixture.file("project/node_modules/nested/package.json");

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

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
            crate::rule::default_rules(),
            std::slice::from_ref(&skip_path),
        );

        assert!(report.matches.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].path, skip_path);
        assert_eq!(report.skipped[0].reason, SkipReason::ConfiguredSkipPath);
    }

    #[test]
    fn scans_relative_roots_after_preparation() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");

        let config = Config {
            roots: vec![Utf8PathBuf::from("project")],
            skip_paths: Vec::new(),
            mode: RunMode::DryRun,
            rules: crate::rule::default_rules(),
        }
        .prepare_with_cwd(&fixture.root())
        .unwrap();

        let report = scan(&config).unwrap();

        assert_eq!(report.roots, vec![fixture.path("project")]);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].path, fixture.path("project/node_modules"));
    }

    #[test]
    fn applies_relative_skip_paths_after_preparation() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");

        let config = Config {
            roots: vec![Utf8PathBuf::from(".")],
            skip_paths: vec![Utf8PathBuf::from("project")],
            mode: RunMode::DryRun,
            rules: crate::rule::default_rules(),
        }
        .prepare_with_cwd(&fixture.root())
        .unwrap();

        let report = scan(&config).unwrap();

        assert!(report.matches.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].path, fixture.path("project"));
    }

    #[test]
    fn skip_paths_do_not_match_same_prefix_siblings() {
        let fixture = Fixture::new();
        fixture.dir("project-other/node_modules");
        fixture.file("project-other/package.json");

        let report = scan_fixture(
            &fixture,
            crate::rule::default_rules(),
            &[fixture.path("project")],
        );

        assert_eq!(report.matches.len(), 1);
        assert_eq!(
            report.matches[0].path,
            fixture.path("project-other/node_modules")
        );
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn nonexistent_roots_report_failures_without_aborting_other_roots() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");

        let config = Config {
            roots: vec![fixture.path("missing"), fixture.path("project")],
            skip_paths: Vec::new(),
            mode: RunMode::DryRun,
            rules: crate::rule::default_rules(),
        }
        .prepare()
        .unwrap();

        let report = scan(&config).unwrap();

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].path, Some(fixture.path("missing")));
    }

    #[test]
    fn file_roots_report_failures_without_aborting_other_roots() {
        let fixture = Fixture::new();
        fixture.file("not-a-directory");
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");

        let config = Config {
            roots: vec![fixture.path("not-a-directory"), fixture.path("project")],
            skip_paths: Vec::new(),
            mode: RunMode::DryRun,
            rules: crate::rule::default_rules(),
        }
        .prepare()
        .unwrap();

        let report = scan(&config).unwrap();

        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(
            report.failures[0].path,
            Some(fixture.path("not-a-directory"))
        );
        assert!(report.failures[0].message.contains("not a directory"));
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
            mode: RunMode::DryRun,
            rules: crate::rule::default_rules(),
        };
        let config = config.prepare().unwrap();
        let report = scan(&config).unwrap();

        assert!(report.matches.is_empty());
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].reason, SkipReason::Symlink);
    }

    #[test]
    fn detects_intermediate_symlink_components_below_a_base_path() {
        let fixture = Fixture::new();
        fixture.dir("real-root/project/node_modules");
        fixture.file("real-root/project/package.json");
        unix_fs::symlink(fixture.path("real-root"), fixture.path("linked-root")).unwrap();

        let symlink =
            symlink_component_between(&fixture.root(), &fixture.path("linked-root/project"), false)
                .unwrap();

        assert_eq!(symlink, Some(fixture.path("linked-root")));
    }

    #[test]
    fn symlinked_evidence_paths_do_not_satisfy_requirements() {
        let fixture = Fixture::new();
        fixture.dir("project/node_modules");
        fixture.dir("real-evidence");
        fixture.file("real-evidence/package.json");
        unix_fs::symlink(
            fixture.path("real-evidence"),
            fixture.path("project/evidence-link"),
        )
        .unwrap();

        let rules = vec![Rule::new(
            "linked-evidence",
            vec![RuleCase::new(
                vec![Target::directory("node_modules")],
                vec![Requirement::any_of(vec![Evidence::candidate_parent(
                    "evidence-link/package.json",
                    EvidenceKind::File,
                )])],
            )],
        )];
        let report = scan_fixture(&fixture, rules, &[]);

        assert!(report.matches.is_empty());
        assert_eq!(report.failures.len(), 1);
        assert_eq!(
            report.failures[0].path,
            Some(fixture.path("project/evidence-link/package.json"))
        );
        assert!(report.failures[0].message.contains("path contains symlink"));
    }

    #[test]
    fn symlinks_do_not_prune_later_siblings() {
        let fixture = Fixture::new();
        fixture.dir("real");
        fixture.dir("project/node_modules");
        fixture.file("project/package.json");
        unix_fs::symlink(fixture.path("real"), fixture.path("linked")).unwrap();

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

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

        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);

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
        let report = scan_fixture(&fixture, crate::rule::default_rules(), &[]);
        fs_err::set_permissions(&blocked_path, Permissions::from_mode(0o700)).unwrap();

        assert_eq!(report.matches.len(), 1);
        assert!(report.failures.iter().any(|failure| {
            failure.path.as_ref() == Some(&blocked_path)
                && failure.message.contains("Permission denied")
        }));
    }

    #[test]
    fn matches_relative_candidate_paths() {
        let target = Target::directory("target");

        assert!(target_matches(Utf8Path::new("./target"), &target));
        assert!(target_matches(Utf8Path::new("target"), &target));
    }

    #[test]
    fn resolves_evidence_for_relative_single_component_candidates() {
        let evidence = Evidence::candidate_parent("Cargo.toml", EvidenceKind::File);

        assert_eq!(
            evidence.resolve_against_target(Utf8Path::new("target"), "target"),
            Utf8PathBuf::from("Cargo.toml")
        );
    }

    #[test]
    fn resolves_target_parent_evidence_for_nested_targets() {
        let evidence = Evidence::target_parent("Gemfile", EvidenceKind::File);

        assert_eq!(
            evidence
                .resolve_against_target(Utf8Path::new("project/vendor/bundle"), "vendor/bundle"),
            Utf8PathBuf::from("project/Gemfile")
        );
    }

    fn scan_fixture(fixture: &Fixture, rules: Vec<Rule>, skip_paths: &[Utf8PathBuf]) -> ScanReport {
        let config = Config {
            roots: vec![fixture.root()],
            skip_paths: skip_paths.to_vec(),
            mode: RunMode::DryRun,
            rules,
        };
        let config = config.prepare().unwrap();

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
