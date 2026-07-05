use std::fmt::{self, Write};

use crate::scan::ScanReport;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportMode {
    DryRun,
}

pub fn render_human_report(report: &ScanReport, mode: ReportMode) -> Result<String, fmt::Error> {
    let mut output = String::new();

    writeln!(output, "Scanning {} root(s)", report.roots.len())?;
    writeln!(output)?;

    match mode {
        ReportMode::DryRun => writeln!(output, "Would exclude:")?,
    }

    if report.matches.is_empty() {
        writeln!(output, "- no matches")?;
    } else {
        for dependency_match in &report.matches {
            writeln!(
                output,
                "- {}  {} + {}",
                dependency_match.path, dependency_match.rule_id, dependency_match.target.path
            )?;
        }
    }

    writeln!(output)?;
    writeln!(output, "Summary:")?;
    writeln!(
        output,
        "{} matched, {} skipped, {} failed",
        report.matches.len(),
        report.skipped.len(),
        report.failures.len()
    )?;

    Ok(output)
}
