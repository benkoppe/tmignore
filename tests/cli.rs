use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn dry_run_reports_matched_dependency_directory() {
    let fixture = Fixture::new();
    fixture.dir("project/node_modules");
    fixture.file("project/package.json");

    let mut command = Command::cargo_bin("tmignore").unwrap();

    command
        .args(["--root", fixture.root()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Dry run: no Time Machine exclusions were changed.")
                .and(predicate::str::contains("Matched directories:"))
                .and(predicate::str::contains("node_modules"))
                .and(predicate::str::contains("    matched: node"))
                .and(predicate::str::contains("    evidence:"))
                .and(predicate::str::contains("package.json"))
                .and(predicate::str::contains("Summary:")),
        );
}

#[test]
fn dry_run_reports_prepared_roots() {
    let fixture = Fixture::new();
    fixture.dir("project/node_modules");
    fixture.file("project/package.json");
    let root = fixture.path_string("project/..");

    let mut command = Command::cargo_bin("tmignore").unwrap();

    command
        .args(["--root", root.as_str()])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "Scanning 1 root:\n- {}",
            fixture.root()
        )));
}

#[cfg(unix)]
#[test]
fn dry_run_groups_skipped_paths_by_default() {
    let fixture = Fixture::new();
    fixture.dir("real/first");
    fixture.dir("real/second");
    fixture.dir(".direnv");
    fixture.symlink("real/first", ".direnv/first");
    fixture.symlink("real/second", ".direnv/second");

    let mut command = Command::cargo_bin("tmignore").unwrap();

    command
        .args(["--root", fixture.root()])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Skipped paths (grouped; use -v to list each path):")
                .and(predicate::str::contains(
                    ".direnv  2 symlinks skipped below this path",
                ))
                .and(predicate::str::contains(".direnv/first  skipped symlink").not())
                .and(predicate::str::contains(".direnv/second  skipped symlink").not()),
        );
}

#[cfg(unix)]
#[test]
fn dry_run_does_not_group_skipped_paths_by_broad_root_child() {
    let fixture = Fixture::new();
    fixture.dir("real/one");
    fixture.dir("real/two");
    fixture.dir("Developer/forks/opencode/.direnv");
    fixture.dir("Developer/forks/opentui/.direnv");
    fixture.symlink("real/one", "Developer/forks/opencode/.direnv/first");
    fixture.symlink("real/two", "Developer/forks/opencode/.direnv/second");
    fixture.symlink("real/one", "Developer/forks/opentui/.direnv/first");
    fixture.symlink("real/two", "Developer/forks/opentui/.direnv/second");

    let mut command = Command::cargo_bin("tmignore").unwrap();
    let root = fixture.path_string("Developer");

    command
        .args(["--root", root.as_str(), "--dry-run"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("forks/opencode/.direnv  2 symlinks skipped below this path")
                .and(predicate::str::contains(
                    "forks/opentui/.direnv  2 symlinks skipped below this path",
                ))
                .and(predicate::str::contains("forks  4 symlinks skipped below this path").not()),
        );
}

#[cfg(unix)]
#[test]
fn dry_run_verbose_lists_every_skipped_path() {
    let fixture = Fixture::new();
    fixture.dir("real/first");
    fixture.dir("real/second");
    fixture.dir(".direnv");
    fixture.symlink("real/first", ".direnv/first");
    fixture.symlink("real/second", ".direnv/second");

    let mut command = Command::cargo_bin("tmignore").unwrap();

    command
        .args(["--root", fixture.root(), "--dry-run", "-v"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("Skipped paths:")
                .and(predicate::str::contains(".direnv/first  skipped symlink"))
                .and(predicate::str::contains(".direnv/second  skipped symlink"))
                .and(predicate::str::contains(".direnv  2 symlinks skipped below this path").not()),
        );
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

    fn root(&self) -> &str {
        self.temp_dir.path().to_str().unwrap()
    }

    fn path(&self, path: &str) -> std::path::PathBuf {
        self.temp_dir.path().join(path)
    }

    fn path_string(&self, path: &str) -> String {
        self.path(path).to_str().unwrap().to_string()
    }

    fn dir(&self, path: &str) {
        fs_err::create_dir_all(self.path(path)).unwrap();
    }

    fn file(&self, path: &str) {
        let path = self.path(path);
        fs_err::create_dir_all(path.parent().unwrap()).unwrap();
        fs_err::write(path, b"").unwrap();
    }

    #[cfg(unix)]
    fn symlink(&self, original: &str, link: &str) {
        std::os::unix::fs::symlink(self.path(original), self.path(link)).unwrap();
    }
}
