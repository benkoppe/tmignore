use std::process::ExitCode;

use clap::Parser;
use tmignore::backend::TmutilBackend;
use tmignore::cli::Cli;
use tmignore::config::RunMode;
use tmignore::report::{ReportMode, ReportOptions, render_human_report};
use tmignore::run::RunReport;
use tmignore::scan::scan;

/// Exit code when the run completed but one or more per-path operations
/// failed, such as an unreadable directory or a `tmutil` error.
const EXIT_PARTIAL_FAILURE: u8 = 1;
/// Exit code when a global precondition failed and no run was performed,
/// such as invalid configuration or missing scan roots.
const EXIT_GLOBAL_FAILURE: u8 = 2;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let report_verbosity = cli.report_verbosity();
    let config = match cli.into_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_GLOBAL_FAILURE);
        }
    };
    let config = match config.prepare() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_GLOBAL_FAILURE);
        }
    };

    let report = match scan(&config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_GLOBAL_FAILURE);
        }
    };

    let report = match config.mode {
        RunMode::DryRun => RunReport::dry_run(report),
        RunMode::Apply => RunReport::apply(report, &TmutilBackend::default()),
    };
    let report_options = ReportOptions::new(ReportMode::from(config.mode), report_verbosity);

    match render_human_report(&report, report_options) {
        Ok(rendered) => {
            print!("{rendered}");
            if report.has_failures() {
                ExitCode::from(EXIT_PARTIAL_FAILURE)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(error) => {
            eprintln!("failed to render report: {error}");
            ExitCode::from(EXIT_GLOBAL_FAILURE)
        }
    }
}
