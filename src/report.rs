use std::collections::BTreeMap;
use std::fmt::{self, Write};

use camino::{Utf8Path, Utf8PathBuf};

use crate::scan::SkippedPath;
use crate::scan::{ScanFailure, ScanReport, SkipReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportMode {
    DryRun,
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
    report: &ScanReport,
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
    }
}

fn render_roots(output: &mut String, report: &ScanReport) -> Result<(), fmt::Error> {
    if report.roots.len() == 1 {
        writeln!(output, "Scanning 1 root:")?;
    } else {
        writeln!(output, "Scanning {} roots:", report.roots.len())?;
    }

    for root in &report.roots {
        writeln!(output, "- {root}")?;
    }

    writeln!(output)
}

fn render_matches(
    output: &mut String,
    report: &ScanReport,
    _mode: ReportMode,
) -> Result<(), fmt::Error> {
    writeln!(output, "Matched directories:")?;

    if report.matches.is_empty() {
        writeln!(output, "- no matches")?;
    } else {
        for dependency_match in &report.matches {
            writeln!(
                output,
                "- {}  {}; evidence: {}",
                dependency_match.path,
                dependency_match.rule_id,
                evidence_label(dependency_match)
            )?;
        }
    }

    writeln!(output)
}

fn evidence_label(dependency_match: &crate::scan::DependencyMatch) -> String {
    if dependency_match.evidence.is_empty() {
        return "none".to_string();
    }

    dependency_match
        .evidence
        .iter()
        .map(|matched_evidence| matched_evidence.path.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_skipped(
    output: &mut String,
    report: &ScanReport,
    verbosity: ReportVerbosity,
) -> Result<(), fmt::Error> {
    if report.skipped.is_empty() {
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
            for skipped_path in &report.skipped {
                render_skipped_path(output, skipped_path)?;
            }
        }
    }

    writeln!(output)
}

fn render_grouped_skipped(output: &mut String, report: &ScanReport) -> Result<(), fmt::Error> {
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

fn render_failures(output: &mut String, report: &ScanReport) -> Result<(), fmt::Error> {
    if report.failures.is_empty() {
        return Ok(());
    }

    writeln!(output, "Failures:")?;

    for failure in &report.failures {
        render_failure(output, failure)?;
    }

    writeln!(output)
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

fn render_summary(output: &mut String, report: &ScanReport) -> Result<(), fmt::Error> {
    writeln!(output, "Summary:")?;
    writeln!(
        output,
        "{} matched, {} skipped, {} failed",
        report.matches.len(),
        report.skipped.len(),
        report.failures.len()
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

fn skipped_groups(report: &ScanReport) -> Vec<SkippedGroup> {
    let mut groups: BTreeMap<(SkipReason, Utf8PathBuf), Vec<SkippedPath>> = BTreeMap::new();

    for skipped_path in &report.skipped {
        let bucket = skipped_bucket(&skipped_path.path, &report.roots);
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

fn skipped_bucket(path: &Utf8Path, roots: &[Utf8PathBuf]) -> Utf8PathBuf {
    roots
        .iter()
        .find_map(|root| bucket_relative_to_root(path, root))
        .unwrap_or_else(|| path.to_path_buf())
}

fn bucket_relative_to_root(path: &Utf8Path, root: &Utf8Path) -> Option<Utf8PathBuf> {
    let relative_path = path.strip_prefix(root).ok()?;
    let first_component = relative_path.components().next()?;

    Some(root.join(first_component.as_str()))
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;
    use crate::rule::{Evidence, EvidenceKind, Target, TargetKind};
    use crate::scan::{DependencyMatch, MatchedEvidence, SkippedPath};

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

        assert!(
            output
                .contains("- /tmp/project/node_modules  node; evidence: /tmp/project/package.json")
        );
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
    fn normal_verbosity_groups_repeated_skipped_paths_by_root_child() {
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

        assert!(output.contains("- ./.direnv  2 symlinks skipped below this path"));
        assert!(output.contains("- ./result  skipped symlink"));
        assert!(!output.contains("./.direnv/flake-inputs/first  skipped symlink"));
        assert!(!output.contains("./.direnv/flake-inputs/second  skipped symlink"));
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
        render_human_report(report, ReportOptions::dry_run(verbosity)).unwrap()
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
}
