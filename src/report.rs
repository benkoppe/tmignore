use std::collections::BTreeMap;
use std::fmt::{self, Write};

use camino::{Utf8Path, Utf8PathBuf};

use crate::config::RunMode;
use crate::run::{ExclusionAction, ExclusionOutcome, RunReport};
use crate::scan::SkippedPath;
use crate::scan::{ScanFailure, SkipReason};

const SYMLINK_GROUP_COMPONENTS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportMode {
    DryRun,
    Apply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportVerbosity {
    Normal,
    Verbose,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportOptions {
    pub mode: ReportMode,
    pub verbosity: ReportVerbosity,
}

impl ReportOptions {
    pub const fn dry_run(verbosity: ReportVerbosity) -> Self {
        Self {
            mode: ReportMode::DryRun,
            verbosity,
        }
    }

    pub const fn new(mode: ReportMode, verbosity: ReportVerbosity) -> Self {
        Self { mode, verbosity }
    }
}

impl From<RunMode> for ReportMode {
    fn from(mode: RunMode) -> Self {
        match mode {
            RunMode::DryRun => Self::DryRun,
            RunMode::Apply => Self::Apply,
        }
    }
}

impl From<u8> for ReportVerbosity {
    fn from(count: u8) -> Self {
        match count {
            0 => Self::Normal,
            1 => Self::Verbose,
            _ => Self::Trace,
        }
    }
}

pub fn render_human_report(
    report: &RunReport,
    options: ReportOptions,
) -> Result<String, fmt::Error> {
    let mut output = String::new();

    render_mode_notice(&mut output, options.mode)?;
    render_roots(&mut output, report)?;
    render_matches(&mut output, report, options.mode)?;
    render_skipped(&mut output, report, options.verbosity)?;
    render_failures(&mut output, report)?;
    render_summary(&mut output, report)?;

    Ok(output)
}

fn render_mode_notice(output: &mut String, mode: ReportMode) -> Result<(), fmt::Error> {
    match mode {
        ReportMode::DryRun => writeln!(output, "Dry run: no Time Machine exclusions were changed."),
        ReportMode::Apply => writeln!(output, "Apply mode: Time Machine exclusions were updated."),
    }
}

fn render_roots(output: &mut String, report: &RunReport) -> Result<(), fmt::Error> {
    if report.scan.roots.len() == 1 {
        writeln!(output, "Scanning 1 root:")?;
    } else {
        writeln!(output, "Scanning {} roots:", report.scan.roots.len())?;
    }

    for root in &report.scan.roots {
        writeln!(output, "- {root}")?;
    }

    writeln!(output)
}

fn render_matches(
    output: &mut String,
    report: &RunReport,
    mode: ReportMode,
) -> Result<(), fmt::Error> {
    writeln!(output, "Matched directories:")?;

    if report.actions.is_empty() {
        writeln!(output, "- no matches")?;
    } else {
        for action in &report.actions {
            writeln!(output, "- {}", action.path)?;
            writeln!(output, "    matched: {}", action.rule_id)?;
            writeln!(output, "    evidence: {}", evidence_label(action))?;

            if mode == ReportMode::Apply {
                writeln!(output, "    action: {}", action_label(&action.outcome))?;
            }
        }
    }

    writeln!(output)
}

fn evidence_label(action: &ExclusionAction) -> String {
    if action.evidence.is_empty() {
        return "none".to_string();
    }

    action
        .evidence
        .iter()
        .map(|matched_evidence| matched_evidence.path.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn action_label(outcome: &ExclusionOutcome) -> &'static str {
    match outcome {
        ExclusionOutcome::DryRun => "would add exclusion",
        ExclusionOutcome::AlreadyExcluded => "already excluded",
        ExclusionOutcome::NewlyExcluded => "added exclusion",
        ExclusionOutcome::StatusFailed(_) => "failed to determine exclusion status",
        ExclusionOutcome::AddFailed(_) => "failed to add exclusion",
    }
}

fn render_skipped(
    output: &mut String,
    report: &RunReport,
    verbosity: ReportVerbosity,
) -> Result<(), fmt::Error> {
    if report.scan.skipped.is_empty() {
        return Ok(());
    }

    match verbosity {
        ReportVerbosity::Normal => {
            writeln!(output, "Skipped paths (grouped; use -v to list each path):")?
        }
        ReportVerbosity::Verbose | ReportVerbosity::Trace => writeln!(output, "Skipped paths:")?,
    }

    match verbosity {
        ReportVerbosity::Normal => render_grouped_skipped(output, report)?,
        ReportVerbosity::Verbose | ReportVerbosity::Trace => {
            for skipped_path in &report.scan.skipped {
                render_skipped_path(output, skipped_path)?;
            }
        }
    }

    writeln!(output)
}

fn render_grouped_skipped(output: &mut String, report: &RunReport) -> Result<(), fmt::Error> {
    for group in skipped_groups(report) {
        if group.paths.len() == 1 {
            render_skipped_path(output, &group.paths[0])?;
        } else {
            writeln!(
                output,
                "- {}  {} {} skipped below this path",
                group.bucket,
                group.paths.len(),
                plural_skip_reason_label(&group.reason)
            )?;
        }
    }

    Ok(())
}

fn render_skipped_path(output: &mut String, skipped_path: &SkippedPath) -> Result<(), fmt::Error> {
    writeln!(
        output,
        "- {}  skipped {}",
        skipped_path.path,
        skip_reason_label(&skipped_path.reason)
    )
}

fn render_failures(output: &mut String, report: &RunReport) -> Result<(), fmt::Error> {
    if report.scan.failures.is_empty() && backend_failures(report).is_empty() {
        return Ok(());
    }

    writeln!(output, "Failures:")?;

    for failure in &report.scan.failures {
        render_failure(output, failure)?;
    }

    for action in backend_failures(report) {
        render_backend_failure(output, action)?;
    }

    writeln!(output)
}

fn backend_failures(report: &RunReport) -> Vec<&ExclusionAction> {
    report
        .actions
        .iter()
        .filter(|action| {
            matches!(
                action.outcome,
                ExclusionOutcome::StatusFailed(_) | ExclusionOutcome::AddFailed(_)
            )
        })
        .collect()
}

fn render_failure(output: &mut String, failure: &ScanFailure) -> Result<(), fmt::Error> {
    match &failure.path {
        Some(path) => {
            writeln!(output, "- {path}")?;
            writeln!(output, "  error: {}", failure.message)
        }
        None => writeln!(output, "- error: {}", failure.message),
    }
}

fn render_backend_failure(output: &mut String, action: &ExclusionAction) -> Result<(), fmt::Error> {
    let diagnostic = match &action.outcome {
        ExclusionOutcome::StatusFailed(diagnostic) | ExclusionOutcome::AddFailed(diagnostic) => {
            diagnostic
        }
        ExclusionOutcome::DryRun
        | ExclusionOutcome::AlreadyExcluded
        | ExclusionOutcome::NewlyExcluded => return Ok(()),
    };

    writeln!(output, "- {}", action.path)?;
    writeln!(output, "  error: {}", diagnostic.message)
}

fn render_summary(output: &mut String, report: &RunReport) -> Result<(), fmt::Error> {
    writeln!(output, "Summary:")?;
    writeln!(
        output,
        "{} matched, {} skipped, {} failed",
        report.actions.len(),
        report.scan.skipped.len(),
        report.scan.failures.len() + backend_failures(report).len()
    )
}

fn skip_reason_label(reason: &SkipReason) -> &'static str {
    match reason {
        SkipReason::ConfiguredSkipPath => "configured skip path",
        SkipReason::Symlink => "symlink",
    }
}

fn plural_skip_reason_label(reason: &SkipReason) -> &'static str {
    match reason {
        SkipReason::ConfiguredSkipPath => "configured skip paths",
        SkipReason::Symlink => "symlinks",
    }
}

#[derive(Debug)]
struct SkippedGroup {
    reason: SkipReason,
    bucket: Utf8PathBuf,
    paths: Vec<SkippedPath>,
}

fn skipped_groups(report: &RunReport) -> Vec<SkippedGroup> {
    let mut groups: BTreeMap<(SkipReason, Utf8PathBuf), Vec<SkippedPath>> = BTreeMap::new();

    for skipped_path in &report.scan.skipped {
        let bucket = skipped_bucket(skipped_path, &report.scan.roots);
        groups
            .entry((skipped_path.reason, bucket))
            .or_default()
            .push(skipped_path.clone());
    }

    groups
        .into_iter()
        .map(|((reason, bucket), paths)| SkippedGroup {
            reason,
            bucket,
            paths,
        })
        .collect()
}

fn skipped_bucket(skipped_path: &SkippedPath, roots: &[Utf8PathBuf]) -> Utf8PathBuf {
    match skipped_path.reason {
        SkipReason::Symlink => symlink_bucket(&skipped_path.path, roots),
        SkipReason::ConfiguredSkipPath => skipped_path.path.clone(),
    }
}

fn symlink_bucket(path: &Utf8Path, roots: &[Utf8PathBuf]) -> Utf8PathBuf {
    let parent = path.parent().unwrap_or(path);

    roots
        .iter()
        .filter_map(|root| bucket_relative_to_root(parent, root, SYMLINK_GROUP_COMPONENTS))
        .max_by_key(|bucket| bucket.as_str().len())
        .unwrap_or_else(|| parent.to_path_buf())
}

fn bucket_relative_to_root(
    path: &Utf8Path,
    root: &Utf8Path,
    max_components: usize,
) -> Option<Utf8PathBuf> {
    let relative_path = path.strip_prefix(root).ok()?;
    let mut components = relative_path.components();
    let first_component = components.next()?;
    let mut bucket = root.join(first_component.as_str());

    for component in components.take(max_components.saturating_sub(1)) {
        bucket.push(component.as_str());
    }

    Some(bucket)
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;
    use crate::backend::BackendDiagnostic;
    use crate::rule::{Evidence, EvidenceKind, Target, TargetKind};
    use crate::scan::{DependencyMatch, MatchedEvidence, ScanReport, SkippedPath};

    #[test]
    fn renders_roots() {
        let report = ScanReport {
            roots: vec![Utf8PathBuf::from("/tmp/projects")],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("Scanning 1 root:\n- /tmp/projects"));
    }

    #[test]
    fn renders_no_matches() {
        let report = ScanReport::default();

        let output = render(&report);

        assert!(output.contains("Dry run: no Time Machine exclusions were changed."));
        assert!(output.contains("Matched directories:\n- no matches"));
    }

    #[test]
    fn renders_match_with_rule_target_and_evidence() {
        let report = ScanReport {
            matches: vec![node_match()],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains(
            "- /tmp/project/node_modules\n    matched: node\n    evidence: /tmp/project/package.json"
        ));
    }

    #[test]
    fn renders_apply_outcomes() {
        let report = RunReport {
            scan: ScanReport::default(),
            actions: vec![action(ExclusionOutcome::NewlyExcluded)],
        };

        let output = render_human_report(
            &report,
            ReportOptions::new(ReportMode::Apply, ReportVerbosity::Normal),
        )
        .unwrap();

        assert!(output.contains("Apply mode: Time Machine exclusions were updated."));
        assert!(output.contains("    action: added exclusion"));
    }

    #[test]
    fn renders_backend_failures() {
        let report = RunReport {
            scan: ScanReport::default(),
            actions: vec![action(ExclusionOutcome::AddFailed(BackendDiagnostic {
                path: Utf8PathBuf::from("/tmp/project/node_modules"),
                message: "tmutil failed".to_string(),
                stdout: String::new(),
                stderr: "bad path".to_string(),
                status_code: Some(1),
            }))],
        };

        let output = render_human_report(
            &report,
            ReportOptions::new(ReportMode::Apply, ReportVerbosity::Normal),
        )
        .unwrap();

        assert!(output.contains("Failures:\n- /tmp/project/node_modules\n  error: tmutil failed"));
        assert!(output.contains("Summary:\n1 matched, 0 skipped, 1 failed"));
    }

    #[test]
    fn renders_skipped_paths() {
        let report = ScanReport {
            skipped: vec![SkippedPath {
                path: Utf8PathBuf::from("/tmp/project/link"),
                reason: SkipReason::Symlink,
            }],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("Skipped paths (grouped; use -v to list each path):\n- /tmp/project/link  skipped symlink"));
    }

    #[test]
    fn normal_verbosity_groups_repeated_skipped_symlinks_by_parent_directory() {
        let report = ScanReport {
            roots: vec![Utf8PathBuf::from(".")],
            skipped: vec![
                SkippedPath {
                    path: Utf8PathBuf::from("./.direnv/flake-inputs/first"),
                    reason: SkipReason::Symlink,
                },
                SkippedPath {
                    path: Utf8PathBuf::from("./.direnv/flake-inputs/second"),
                    reason: SkipReason::Symlink,
                },
                SkippedPath {
                    path: Utf8PathBuf::from("./result"),
                    reason: SkipReason::Symlink,
                },
            ],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("- ./.direnv/flake-inputs  2 symlinks skipped below this path"));
        assert!(output.contains("- ./result  skipped symlink"));
        assert!(!output.contains("./.direnv/flake-inputs/first  skipped symlink"));
        assert!(!output.contains("./.direnv/flake-inputs/second  skipped symlink"));
    }

    #[test]
    fn normal_verbosity_does_not_group_symlinks_by_broad_scan_root_child() {
        let report = ScanReport {
            roots: vec![Utf8PathBuf::from("../")],
            skipped: vec![
                SkippedPath {
                    path: Utf8PathBuf::from("../forks/opencode/.direnv/first"),
                    reason: SkipReason::Symlink,
                },
                SkippedPath {
                    path: Utf8PathBuf::from("../forks/opencode/.direnv/second"),
                    reason: SkipReason::Symlink,
                },
                SkippedPath {
                    path: Utf8PathBuf::from("../forks/opentui/.direnv/first"),
                    reason: SkipReason::Symlink,
                },
                SkippedPath {
                    path: Utf8PathBuf::from("../forks/opentui/.direnv/second"),
                    reason: SkipReason::Symlink,
                },
            ],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("- ../forks/opencode/.direnv  2 symlinks skipped below this path"));
        assert!(output.contains("- ../forks/opentui/.direnv  2 symlinks skipped below this path"));
        assert!(!output.contains("- ../forks  4 symlinks skipped below this path"));
    }

    #[test]
    fn verbose_verbosity_renders_every_skipped_path() {
        let report = ScanReport {
            roots: vec![Utf8PathBuf::from(".")],
            skipped: vec![
                SkippedPath {
                    path: Utf8PathBuf::from("./.direnv/flake-inputs/first"),
                    reason: SkipReason::Symlink,
                },
                SkippedPath {
                    path: Utf8PathBuf::from("./.direnv/flake-inputs/second"),
                    reason: SkipReason::Symlink,
                },
            ],
            ..ScanReport::default()
        };

        let output = render_with_verbosity(&report, ReportVerbosity::Verbose);

        assert!(output.contains("- ./.direnv/flake-inputs/first  skipped symlink"));
        assert!(output.contains("- ./.direnv/flake-inputs/second  skipped symlink"));
        assert!(!output.contains("- ./.direnv  2 symlinks skipped below this path"));
    }

    #[test]
    fn grouped_skipped_summary_preserves_raw_count() {
        let report = ScanReport {
            roots: vec![Utf8PathBuf::from(".")],
            skipped: vec![
                SkippedPath {
                    path: Utf8PathBuf::from("./.direnv/first"),
                    reason: SkipReason::Symlink,
                },
                SkippedPath {
                    path: Utf8PathBuf::from("./.direnv/second"),
                    reason: SkipReason::Symlink,
                },
            ],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("Summary:\n0 matched, 2 skipped, 0 failed"));
    }

    #[test]
    fn renders_failures_with_path() {
        let report = ScanReport {
            failures: vec![ScanFailure {
                path: Some(Utf8PathBuf::from("/tmp/project")),
                message: "permission denied".to_string(),
            }],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("Failures:\n- /tmp/project\n  error: permission denied"));
    }

    #[test]
    fn renders_failures_without_path() {
        let report = ScanReport {
            failures: vec![ScanFailure {
                path: None,
                message: "path is not valid UTF-8".to_string(),
            }],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("Failures:\n- error: path is not valid UTF-8"));
    }

    #[test]
    fn renders_summary_counts() {
        let report = ScanReport {
            matches: vec![node_match()],
            skipped: vec![SkippedPath {
                path: Utf8PathBuf::from("/tmp/project/link"),
                reason: SkipReason::Symlink,
            }],
            failures: vec![ScanFailure {
                path: Some(Utf8PathBuf::from("/tmp/project")),
                message: "permission denied".to_string(),
            }],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("Summary:\n1 matched, 1 skipped, 1 failed"));
    }

    fn render(report: &ScanReport) -> String {
        render_with_verbosity(report, ReportVerbosity::Normal)
    }

    fn render_with_verbosity(report: &ScanReport, verbosity: ReportVerbosity) -> String {
        let report = RunReport::dry_run(report.clone());
        render_human_report(&report, ReportOptions::dry_run(verbosity)).unwrap()
    }

    fn node_match() -> DependencyMatch {
        DependencyMatch {
            path: Utf8PathBuf::from("/tmp/project/node_modules"),
            rule_id: "node",
            target: Target {
                path: "node_modules",
                kind: TargetKind::Directory,
            },
            evidence: vec![MatchedEvidence {
                evidence: Evidence::candidate_parent("package.json", EvidenceKind::File),
                path: Utf8PathBuf::from("/tmp/project/package.json"),
            }],
        }
    }

    fn action(outcome: ExclusionOutcome) -> ExclusionAction {
        let dependency_match = node_match();

        ExclusionAction {
            path: dependency_match.path,
            rule_id: dependency_match.rule_id,
            target: dependency_match.target,
            evidence: dependency_match.evidence,
            outcome,
        }
    }
}
