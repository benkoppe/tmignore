use std::process::ExitCode;

use clap::Parser;
use tmignore::cli::Cli;
use tmignore::report::{ReportMode, render_human_report};
use tmignore::scan::scan;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let config = cli.into_config();

    if !config.dry_run {
        eprintln!("tmignore only supports --dry-run until the Time Machine backend is implemented");
        return ExitCode::FAILURE;
    }

    let report = match scan(&config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    match render_human_report(&report, ReportMode::DryRun) {
        Ok(rendered) => {
            print!("{rendered}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to render report: {error}");
            ExitCode::FAILURE
        }
    }
}
