use std::fmt::{self, Write};

use crate::scan::{ScanFailure, ScanReport, SkipReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportMode {
    DryRun,
}

pub fn render_human_report(report: &ScanReport, mode: ReportMode) -> Result<String, fmt::Error> {
    let mut output = String::new();

    render_roots(&mut output, report)?;
    render_matches(&mut output, report, mode)?;
    render_skipped(&mut output, report)?;
    render_failures(&mut output, report)?;
    render_summary(&mut output, report)?;

    Ok(output)
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
    mode: ReportMode,
) -> Result<(), fmt::Error> {
    match mode {
        ReportMode::DryRun => writeln!(output, "Would exclude:")?,
    }

    if report.matches.is_empty() {
        writeln!(output, "- no matches")?;
    } else {
        for dependency_match in &report.matches {
            writeln!(output, "- {}", dependency_match.path)?;
            writeln!(output, "  rule: {}", dependency_match.rule_id)?;
            writeln!(output, "  target: {}", dependency_match.target.path)?;

            if dependency_match.evidence.is_empty() {
                writeln!(output, "  evidence: none")?;
            } else {
                writeln!(output, "  evidence:")?;

                for matched_evidence in &dependency_match.evidence {
                    writeln!(output, "  - {}", matched_evidence.path)?;
                }
            }
        }
    }

    writeln!(output)
}

fn render_skipped(output: &mut String, report: &ScanReport) -> Result<(), fmt::Error> {
    if report.skipped.is_empty() {
        return Ok(());
    }

    writeln!(output, "Skipped:")?;

    for skipped_path in &report.skipped {
        writeln!(
            output,
            "- {}  {}",
            skipped_path.path,
            skip_reason_label(&skipped_path.reason)
        )?;
    }

    writeln!(output)
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

        assert!(output.contains("Would exclude:\n- no matches"));
    }

    #[test]
    fn renders_match_with_rule_target_and_evidence() {
        let report = ScanReport {
            matches: vec![node_match()],
            ..ScanReport::default()
        };

        let output = render(&report);

        assert!(output.contains("- /tmp/project/node_modules"));
        assert!(output.contains("  rule: node"));
        assert!(output.contains("  target: node_modules"));
        assert!(output.contains("  evidence:\n  - /tmp/project/package.json"));
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

        assert!(output.contains("Skipped:\n- /tmp/project/link  symlink"));
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
        render_human_report(report, ReportMode::DryRun).unwrap()
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
