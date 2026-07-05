use std::process::ExitCode;

use clap::Parser;
use tmignore::backend::TmutilBackend;
use tmignore::cli::{AllArgs, Cli, Command, CommonArgs, GlobalArgs, ScanArgs};
use tmignore::config::{AppConfig, ConfigError, RunMode};
use tmignore::global::{GlobalRunReport, scan_global};
use tmignore::report::{
    ReportMode, ReportOptions, render_all_human_report, render_global_human_report,
    render_human_report,
};
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

    match cli.command {
        Command::Scan(args) => run_scan(args),
        Command::Global(args) => run_global(args),
        Command::All(args) => run_all(args),
    }
}

fn run_scan(args: ScanArgs) -> ExitCode {
    let mode = args.common.run_mode();
    let report_verbosity = args.common.report_verbosity();
    let scan_config = match AppConfig::load_scan(args.common.config.as_deref(), mode) {
        Ok(config) => config,
        Err(error) => return global_failure(error),
    };
    let scan_config = scan_config.with_cli_paths(args.root, args.skip);

    let scan_config = match scan_config.prepare() {
        Ok(config) => config,
        Err(error) => return global_failure(error),
    };
    let report = match scan(&scan_config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_GLOBAL_FAILURE);
        }
    };
    let report = run_scan_report(report, mode);

    render_and_exit_scan(&report, mode, report_verbosity)
}

fn run_global(args: GlobalArgs) -> ExitCode {
    let mode = args.common.run_mode();
    let report_verbosity = args.common.report_verbosity();
    let global_config = match AppConfig::load_global(args.common.config.as_deref()) {
        Ok(config) => config,
        Err(error) => return global_failure(error),
    };
    let global_config = match global_config.prepare() {
        Ok(config) => config,
        Err(error) => return global_failure(error),
    };
    let report = run_global_report(scan_global(&global_config), mode);

    render_and_exit_global(&report, mode, report_verbosity)
}

fn run_all(args: AllArgs) -> ExitCode {
    let mode = args.common.run_mode();
    let report_verbosity = args.common.report_verbosity();
    let app_config = match load_app_config(&args.common, mode) {
        Ok(config) => config,
        Err(error) => return global_failure(error),
    };
    let scan_config = app_config.scan.with_cli_paths(args.root, args.skip);

    let scan_config = match scan_config.prepare() {
        Ok(config) => config,
        Err(error) => return global_failure(error),
    };
    let global_config = match app_config.global.prepare() {
        Ok(config) => config,
        Err(error) => return global_failure(error),
    };

    let scan_report = match scan(&scan_config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(EXIT_GLOBAL_FAILURE);
        }
    };
    let scan_report = run_scan_report(scan_report, mode);
    let global_report = run_global_report(scan_global(&global_config), mode);

    render_and_exit_all(&scan_report, &global_report, mode, report_verbosity)
}

fn load_app_config(args: &CommonArgs, mode: RunMode) -> Result<AppConfig, ConfigError> {
    AppConfig::load(args.config.as_deref(), mode)
}

fn run_scan_report(scan_report: tmignore::ScanReport, mode: RunMode) -> RunReport {
    match mode {
        RunMode::DryRun => RunReport::dry_run(scan_report),
        RunMode::Apply => RunReport::apply(scan_report, &TmutilBackend::default()),
    }
}

fn run_global_report(global_report: tmignore::GlobalScanReport, mode: RunMode) -> GlobalRunReport {
    match mode {
        RunMode::DryRun => GlobalRunReport::dry_run(global_report),
        RunMode::Apply => GlobalRunReport::apply(global_report, &TmutilBackend::default()),
    }
}

fn render_and_exit_scan(
    report: &RunReport,
    mode: RunMode,
    verbosity: tmignore::report::ReportVerbosity,
) -> ExitCode {
    match render_human_report(
        report,
        ReportOptions::new(ReportMode::from(mode), verbosity),
    ) {
        Ok(rendered) => {
            print!("{rendered}");
            partial_failure_if(report.has_failures())
        }
        Err(error) => render_failure(error),
    }
}

fn render_and_exit_global(
    report: &GlobalRunReport,
    mode: RunMode,
    verbosity: tmignore::report::ReportVerbosity,
) -> ExitCode {
    match render_global_human_report(
        report,
        ReportOptions::new(ReportMode::from(mode), verbosity),
    ) {
        Ok(rendered) => {
            print!("{rendered}");
            partial_failure_if(report.has_failures())
        }
        Err(error) => render_failure(error),
    }
}

fn render_and_exit_all(
    scan_report: &RunReport,
    global_report: &GlobalRunReport,
    mode: RunMode,
    verbosity: tmignore::report::ReportVerbosity,
) -> ExitCode {
    match render_all_human_report(
        scan_report,
        global_report,
        ReportOptions::new(ReportMode::from(mode), verbosity),
    ) {
        Ok(rendered) => {
            print!("{rendered}");
            partial_failure_if(scan_report.has_failures() || global_report.has_failures())
        }
        Err(error) => render_failure(error),
    }
}

fn partial_failure_if(has_failures: bool) -> ExitCode {
    if has_failures {
        ExitCode::from(EXIT_PARTIAL_FAILURE)
    } else {
        ExitCode::SUCCESS
    }
}

fn global_failure(error: ConfigError) -> ExitCode {
    eprintln!("{error}");
    ExitCode::from(EXIT_GLOBAL_FAILURE)
}

fn render_failure(error: std::fmt::Error) -> ExitCode {
    eprintln!("failed to render report: {error}");
    ExitCode::from(EXIT_GLOBAL_FAILURE)
}
