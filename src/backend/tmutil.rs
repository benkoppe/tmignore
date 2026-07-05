use std::io;
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};

use crate::backend::{BackendDiagnostic, ExclusionChange, ExclusionStatus, TimeMachineBackend};

const DEFAULT_TMUTIL_PATH: &str = "/usr/bin/tmutil";
const INCLUDED_MARKER: &str = "[Included]";
const EXCLUDED_MARKER: &str = "[Excluded]";
const KNOWN_TMUTIL_ERROR_CODES: &[&str] = &["-20", "-50"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub status_code: Option<i32>,
    pub success: bool,
}

pub trait CommandRunner {
    fn run(&self, program: &Utf8Path, args: &[&str]) -> io::Result<CommandOutput>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessCommandRunner;

impl CommandRunner for ProcessCommandRunner {
    fn run(&self, program: &Utf8Path, args: &[&str]) -> io::Result<CommandOutput> {
        let output = Command::new(program).args(args).output()?;

        Ok(CommandOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            status_code: output.status.code(),
            success: output.status.success(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct TmutilBackend<R = ProcessCommandRunner> {
    tmutil_path: Utf8PathBuf,
    runner: R,
}

impl Default for TmutilBackend<ProcessCommandRunner> {
    fn default() -> Self {
        Self::new(ProcessCommandRunner)
    }
}

impl<R> TmutilBackend<R> {
    pub fn new(runner: R) -> Self {
        Self {
            tmutil_path: Utf8PathBuf::from(DEFAULT_TMUTIL_PATH),
            runner,
        }
    }

    pub fn with_tmutil_path(mut self, tmutil_path: impl Into<Utf8PathBuf>) -> Self {
        self.tmutil_path = tmutil_path.into();
        self
    }
}

impl<R: CommandRunner> TimeMachineBackend for TmutilBackend<R> {
    fn exclusion_status(&self, path: &Utf8Path) -> ExclusionStatus {
        match self
            .runner
            .run(&self.tmutil_path, &["isexcluded", path.as_str()])
        {
            Ok(output) if output.success => parse_isexcluded_output(path, output),
            Ok(output) => ExclusionStatus::Unknown(diagnostic_from_output(
                path,
                "tmutil isexcluded failed",
                output,
            )),
            Err(error) => ExclusionStatus::Unknown(diagnostic_from_io_error(
                path,
                "failed to run tmutil isexcluded",
                error,
            )),
        }
    }

    fn add_exclusion(&self, path: &Utf8Path) -> Result<ExclusionChange, BackendDiagnostic> {
        match self
            .runner
            .run(&self.tmutil_path, &["addexclusion", path.as_str()])
        {
            Ok(output) if output.success => Ok(ExclusionChange::NewlyExcluded),
            Ok(output) => Err(diagnostic_from_output(
                path,
                "tmutil addexclusion failed",
                output,
            )),
            Err(error) => Err(diagnostic_from_io_error(
                path,
                "failed to run tmutil addexclusion",
                error,
            )),
        }
    }
}

fn parse_isexcluded_output(path: &Utf8Path, output: CommandOutput) -> ExclusionStatus {
    if output.stdout.contains(EXCLUDED_MARKER) {
        ExclusionStatus::Excluded
    } else if output.stdout.contains(INCLUDED_MARKER) {
        ExclusionStatus::Included
    } else {
        ExclusionStatus::Unknown(diagnostic_from_output(
            path,
            "tmutil isexcluded returned unrecognized output",
            output,
        ))
    }
}

fn diagnostic_from_output(
    path: &Utf8Path,
    context: &'static str,
    output: CommandOutput,
) -> BackendDiagnostic {
    BackendDiagnostic {
        path: path.to_path_buf(),
        message: diagnostic_message(context, output.status_code, &output.stdout, &output.stderr),
        stdout: output.stdout,
        stderr: output.stderr,
        status_code: output.status_code,
    }
}

fn diagnostic_from_io_error(
    path: &Utf8Path,
    context: &'static str,
    error: io::Error,
) -> BackendDiagnostic {
    BackendDiagnostic {
        path: path.to_path_buf(),
        message: format!("{context}: {error}"),
        stdout: String::new(),
        stderr: String::new(),
        status_code: None,
    }
}

fn diagnostic_message(
    context: &'static str,
    status_code: Option<i32>,
    stdout: &str,
    stderr: &str,
) -> String {
    let mut message = context.to_string();

    if let Some(status_code) = status_code {
        message.push_str(&format!(" (exit status {status_code})"));
    }

    if let Some(error_code) = known_tmutil_error_code(stdout, stderr) {
        message.push_str(&format!("; tmutil reported error {error_code}"));
    }

    message
}

fn known_tmutil_error_code(stdout: &str, stderr: &str) -> Option<&'static str> {
    KNOWN_TMUTIL_ERROR_CODES
        .iter()
        .copied()
        .find(|code| contains_error_code(stdout, code) || contains_error_code(stderr, code))
}

/// Returns whether `text` contains `code` as a standalone signed number, so
/// that `-20` is found in `error -20` or `Error (-20)` but not in `-200` or
/// `x-20y`.
fn contains_error_code(text: &str, code: &str) -> bool {
    text.match_indices(code).any(|(index, _)| {
        let before = text[..index].chars().next_back();
        let after = text[index + code.len()..].chars().next();

        let boundary_before = before.is_none_or(|character| !character.is_ascii_alphanumeric());
        let boundary_after = after.is_none_or(|character| !character.is_ascii_digit());

        boundary_before && boundary_after
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;

    #[test]
    fn parses_excluded_status() {
        let backend = backend_with_output(CommandOutput::success(
            "[Excluded]      /tmp/project/node_modules\n",
        ));

        assert_eq!(
            backend.exclusion_status(Utf8Path::new("/tmp/project/node_modules")),
            ExclusionStatus::Excluded
        );
    }

    #[test]
    fn parses_included_status() {
        let backend = backend_with_output(CommandOutput::success(
            "[Included]      /tmp/project/node_modules\n",
        ));

        assert_eq!(
            backend.exclusion_status(Utf8Path::new("/tmp/project/node_modules")),
            ExclusionStatus::Included
        );
    }

    #[test]
    fn reports_status_command_failure() {
        let backend = backend_with_output(CommandOutput::failure(
            "",
            "tmutil: error -20 while changing exclusions\n",
            Some(1),
        ));

        let ExclusionStatus::Unknown(diagnostic) =
            backend.exclusion_status(Utf8Path::new("/tmp/project/node_modules"))
        else {
            panic!("expected unknown status");
        };

        assert!(diagnostic.message.contains("tmutil isexcluded failed"));
        assert!(diagnostic.message.contains("error -20"));
        assert_eq!(diagnostic.status_code, Some(1));
        assert!(diagnostic.stderr.contains("-20"));
    }

    #[test]
    fn reports_unrecognized_status_output() {
        let backend = backend_with_output(CommandOutput::success("unexpected output\n"));

        let ExclusionStatus::Unknown(diagnostic) =
            backend.exclusion_status(Utf8Path::new("/tmp/project/node_modules"))
        else {
            panic!("expected unknown status");
        };

        assert!(diagnostic.message.contains("unrecognized output"));
        assert_eq!(diagnostic.stdout, "unexpected output\n");
    }

    #[test]
    fn adds_exclusion() {
        let backend = backend_with_output(CommandOutput::success(""));

        assert_eq!(
            backend.add_exclusion(Utf8Path::new("/tmp/project/node_modules")),
            Ok(ExclusionChange::NewlyExcluded)
        );
    }

    #[test]
    fn reports_add_exclusion_failure() {
        let backend = backend_with_output(CommandOutput::failure(
            "partial stdout\n",
            "tmutil: error -50 while changing exclusions\n",
            Some(2),
        ));

        let diagnostic = backend
            .add_exclusion(Utf8Path::new("/tmp/project/node_modules"))
            .unwrap_err();

        assert!(diagnostic.message.contains("tmutil addexclusion failed"));
        assert!(diagnostic.message.contains("error -50"));
        assert_eq!(diagnostic.status_code, Some(2));
        assert_eq!(diagnostic.stdout, "partial stdout\n");
        assert!(diagnostic.stderr.contains("-50"));
    }

    #[test]
    fn recognizes_known_error_codes_only_as_standalone_numbers() {
        assert_eq!(
            known_tmutil_error_code("", "tmutil: error -20 while changing exclusions\n"),
            Some("-20")
        );
        assert_eq!(
            known_tmutil_error_code("", "Error (-50) while attempting to change exclusions.\n"),
            Some("-50")
        );
        assert_eq!(
            known_tmutil_error_code("", "tmutil: error -200 while changing exclusions\n"),
            None
        );
        assert_eq!(
            known_tmutil_error_code("/tmp/build-2024/node_modules\n", ""),
            None
        );
        assert_eq!(known_tmutil_error_code("", "error x-20y\n"), None);
    }

    #[test]
    fn invokes_tmutil_directly_with_path_argument() {
        let runner = FakeRunner::new(vec![CommandResult::Output(CommandOutput::success(
            "[Included]      /tmp/project with spaces/node_modules\n",
        ))]);
        let backend = TmutilBackend::new(runner).with_tmutil_path("/custom/tmutil");

        assert_eq!(
            backend.exclusion_status(Utf8Path::new("/tmp/project with spaces/node_modules")),
            ExclusionStatus::Included
        );

        assert_eq!(
            backend.runner.calls.borrow().as_slice(),
            &[CommandCall {
                program: Utf8PathBuf::from("/custom/tmutil"),
                args: vec![
                    "isexcluded".to_string(),
                    "/tmp/project with spaces/node_modules".to_string(),
                ],
            }]
        );
    }

    fn backend_with_output(output: CommandOutput) -> TmutilBackend<FakeRunner> {
        TmutilBackend::new(FakeRunner::new(vec![CommandResult::Output(output)]))
    }

    impl CommandOutput {
        fn success(stdout: &str) -> Self {
            Self {
                stdout: stdout.to_string(),
                stderr: String::new(),
                status_code: Some(0),
                success: true,
            }
        }

        fn failure(stdout: &str, stderr: &str, status_code: Option<i32>) -> Self {
            Self {
                stdout: stdout.to_string(),
                stderr: stderr.to_string(),
                status_code,
                success: false,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct CommandCall {
        program: Utf8PathBuf,
        args: Vec<String>,
    }

    enum CommandResult {
        Output(CommandOutput),
    }

    struct FakeRunner {
        results: RefCell<VecDeque<CommandResult>>,
        calls: RefCell<Vec<CommandCall>>,
    }

    impl FakeRunner {
        fn new(results: Vec<CommandResult>) -> Self {
            Self {
                results: RefCell::new(results.into()),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &Utf8Path, args: &[&str]) -> io::Result<CommandOutput> {
            self.calls.borrow_mut().push(CommandCall {
                program: program.to_path_buf(),
                args: args.iter().map(|arg| (*arg).to_string()).collect(),
            });

            match self.results.borrow_mut().pop_front() {
                Some(CommandResult::Output(output)) => Ok(output),
                None => panic!("unexpected command invocation"),
            }
        }
    }
}
