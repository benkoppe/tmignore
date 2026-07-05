# AGENTS.md

This repository is intended to become `tmignore`, a small, auditable macOS utility for finding development dependency/cache directories and excluding them from Time Machine backups. Ignore the current repository name if it still contains `asimov-rs`; the intended product name is `tmignore`.

Always read `GUIDELINES.md` before making decisions in this repository. Its instructions are project policy, especially the requirement to prefer the most correct architecture over the smallest compatibility-preserving change. The project is pre-alpha; do not preserve old behavior unless the user explicitly asks for it.

## Project Intent

`tmignore` exists because Time Machine is useful, but developer workstations accumulate huge restoreable dependency directories such as `node_modules`, `target`, `vendor`, `.venv`, and build caches. Backing these directories up wastes disk, bandwidth, and backup time. The tool should automatically detect these directories when they are clearly project dependencies or generated caches, then mark them as excluded from Time Machine.

This is intentionally not the same model as `.gitignore`. Files that are inappropriate for Git may still be important to back up locally. For example, `.env` files, editor-local configuration, private notes, and other user-created project files should not be excluded merely because they are untracked, secret, machine-specific, or commonly gitignored. `tmignore` should target restoreable dependency/cache directories, not every file or directory that a VCS ignore rule might hide.

The goal is not to blindly port an existing script. The goal is to build a trusted, explicit, Nix-friendly replacement that the user is comfortable running automatically on their own macOS machine.

The intended deployment model is nix-darwin. The tool should be easy to package as a Rust binary and run on a schedule through declarative launchd configuration managed by Nix.

## Upstream Reference

The original inspiration is `asimov` by Steve Grunwell. A local reference checkout may be available at:

`/Users/ben/.local/share/opencode/repos/github.com/stevegrunwell/asimov`

When working on this project, inspect upstream `asimov` for context, behavior, and prior art, especially:

- `README.md` for the problem statement and user-facing behavior.
- `asimov` for the shell implementation and dependency/sentinel rule list.
- `com.stevegrunwell.asimov.plist` for the original launchd scheduling model.
- `install.sh` for manual installation behavior, but do not copy its installation strategy.
- `tests/` for existing behavior coverage and gaps.

Important upstream observations:

- `asimov` is a small Bash wrapper around `find`, `tmutil`, `grep`, and `du`.
- It scans the user's home directory, skipping only `~/.Trash` and `~/Library`.
- It uses `find -prune` to avoid walking into matched dependency directories after discovering them.
- It only excludes directories when a paired sentinel file exists, such as `node_modules` next to `package.json`.
- Its tests cover basic matching, multiple matches, idempotence, and skipping trash, but not modern macOS Time Machine behavior, permissions, launchd behavior, symlinks, app bundles, partial failures, or `tmutil` error semantics.
- It has open GitHub issues around unresolved `tmutil` errors, modern macOS/ARM behavior, service bootstrap confusion, custom vendor dirs, and maintainer succession.

Relevant upstream issues observed during initial evaluation include:

- `stevegrunwell/asimov#99`: looking for a new maintainer.
- `stevegrunwell/asimov#101`: `tmutil` error `-20` while changing exclusions.
- `stevegrunwell/asimov#86`: `tmutil` error `-50` while changing exclusions.
- `stevegrunwell/asimov#89`: Monterey/ARM report where exclusions appear not to show via `mdfind`.
- `stevegrunwell/asimov#82`: launchd/service bootstrap confusion.
- `stevegrunwell/asimov#95`: custom vendor directories.
- `stevegrunwell/asimov#75`: only run inside current directory.
- `stevegrunwell/asimov#84`: summary of ignored size.

These issues are part of the rationale for owning a small rewrite rather than trusting the upstream package or a future upstream maintainer handoff.

## Rust Choice

Rust is an appropriate implementation language for this project, not because the scanning workload demands maximum performance, but because it supports the desired trust and deployment properties:

- A single compiled binary is easy to package and run through Nix/nix-darwin.
- Filesystem traversal can be explicit, typed, and testable.
- Rule definitions can be structured instead of shell strings.
- Process execution around `tmutil` can avoid shell interpolation and capture status/stdout/stderr precisely.
- Failure handling can be per-path instead of process-global.
- Dry-run, structured output, and testability are straightforward.

Prefer well-known Rust crates where they improve correctness or maintainability. Follow `GUIDELINES.md` and search current dependency versions with Cargo before recommending or adding dependencies.

## Core Behavior

The core behavior is:

- Walk configured filesystem roots.
- Identify dependency/cache directories by explicit rule cases that pair candidate targets with required filesystem evidence.
- Exclude matched directories from Time Machine using `tmutil`.
- Avoid descending into directories that have already been matched for exclusion.
- Avoid descending into explicitly skipped paths.
- Report actions, skipped paths, failures, and summary information clearly.

The scanner must prune matched directories. For example, if it finds `node_modules` and the active rule case's requirements are satisfied by a nearby `package.json`, it should record and process that `node_modules` path, then not walk inside it. This prevents wasted traversal and avoids nested false positives inside vendored dependency trees.

The scanner should not follow symlinks by default. Symlink behavior must be explicit, documented, and tested if support is added.

The scanner should prefer explicit configured roots over scanning all of `~`. Reasonable roots might include development directories such as `~/Developer`, `~/Code`, `~/Projects`, or user-specified paths. Scanning the entire home directory by default is too broad and can hit app bundles, language-manager internals, SDKs, permissions boundaries, and other undesirable locations.

The tool should continue after ordinary per-path failures. A single `tmutil`, metadata, permissions, or size-calculation failure should not abort the entire run unless it indicates a global precondition failure.

## Time Machine Backend

Time Machine exclusion is the primary backend and the initial reason for the project.

Use Apple's supported `tmutil` interface unless there is a strong, researched reason not to. The Rust binary should invoke `tmutil` directly without a shell. It should treat `tmutil` status, stdout, and stderr as important diagnostics.

The implementation should distinguish at least these concepts:

- Already excluded.
- Newly excluded.
- Failed to determine exclusion status.
- Failed to add exclusion.
- Path skipped by configuration.
- Path matched by rule but not processed due to dry-run.

`tmutil` error codes seen in upstream issues, especially `-20` and `-50`, should be handled as reportable diagnostics. Do not hide them behind generic failure messages.

Idempotence is important. Re-running the tool should not repeatedly add the same exclusions or produce misleading output.

## Optional Indexing Backends

It is reasonable for the project to eventually offer optional support for also excluding matched paths from Spotlight-backed indexers such as Spotlight itself, Alfred, and Raycast. This must not be conflated with the Time Machine backend.

Treat Time Machine exclusion as the core feature. Treat Spotlight/Alfred/Raycast behavior as optional backends or future capabilities with separate configuration and dry-run reporting.

Do not assume Spotlight has a clean equivalent to `tmutil addexclusion`. Some approaches may be global, UI-mediated, privacy-sensitive, volume-level, or hacks such as renaming paths with `.noindex`; those should not be adopted casually. Do not change project directory names to influence indexing unless the user explicitly requests that behavior.

If optional indexing backends are added later, they should be reliable, documented, reversible where possible, and visibly separate from Time Machine actions in output.

## Configuration Expectations

Configuration should be explicit, auditable, and friendly to Nix generation.

The tool should support configurable roots, skip paths, and dependency rules. Built-in defaults are acceptable, but users should be able to see and override them.

Dependency rules should be modeled as structured records, not as simple directory/sentinel pairs. A rule should have a stable identifier and one or more cases. Each case should declare candidate targets plus evidence requirements that must be satisfied before the target is considered safe to exclude.

Rule semantics should use this shape unless a better design is deliberately chosen during implementation:

- A rule matches when any of its cases matches.
- A case matches when the candidate path matches any target in the case and every requirement is satisfied.
- A requirement is satisfied when any one of its evidence entries exists.
- Multiple requirements express logical AND.
- Multiple evidence entries inside one requirement express logical OR.
- Evidence may be a file, directory, or either, and should declare whether it is relative to the candidate target or the candidate's parent.
- Exclusion targets should remain directories by default; file targets are out of scope unless explicitly approved later.

This model allows one rule case to cover multiple equivalent target directories, such as `.venv` and `venv`, while also supporting stricter cases that require multiple independent pieces of evidence.

Examples of desired default rule behavior include:

- `node_modules` with `package.json`.
- `target` with `Cargo.toml`.
- `vendor` with `composer.json`, `Gemfile`, or `go.mod`.
- `.venv` or `venv` with Python evidence such as `pyproject.toml` or `requirements.txt`.
- `.build` with Swift or Elixir evidence.
- `.gradle` or `build` with Gradle evidence.
- `.dart_tool`, `.packages`, or `build` with Dart/Flutter evidence.
- `.stack-work` with `stack.yaml`.
- `.tox` with `tox.ini`.
- `.nox` with `noxfile.py`.
- `.vagrant` with `Vagrantfile`.
- `Carthage` with `Cartfile`.
- `Pods` with `Podfile`.
- `.parcel-cache` with `package.json`.
- `.terraform.d` with `.terraformrc`.
- `.terragrunt-cache` with `terragrunt.hcl`.
- `cdk.out` with `cdk.json`.

The exact rule set should be chosen deliberately during implementation. Do not blindly copy every upstream rule if it is too broad or likely to create false positives. The pre-alpha state permits better defaults and richer rule semantics even if they differ from `asimov`.

## CLI Expectations

The CLI should be safe for unattended scheduled use and understandable when run manually.

Expected capabilities include:

- A dry-run mode that shows what would be excluded without modifying Time Machine state.
- A way to specify scan roots.
- A way to specify or load configuration.
- Clear reporting of matched, excluded, already-excluded, skipped, and failed paths.
- A summary suitable for launchd logs.
- Non-zero exit behavior that distinguishes global failure from partial per-path failures, if practical.

JSON or other structured output may be useful for automation, but human-readable output should remain good because this is a local maintenance tool.

## Nix And macOS Expectations

The project should be designed for nix-darwin from the beginning.

Use `flake-parts` for Nix structure, per `GUIDELINES.md`. The binary should be packageable through Nix and easy to wire into a nix-darwin launchd job.

Do not prioritize Homebrew or manual installer scripts. Upstream `asimov` uses Homebrew/manual install and a bundled plist; those are context, not the target distribution model.

launchd integration should account for modern macOS behavior and produce useful logs. Service bootstrap confusion in upstream issues is a warning that service management should be explicit and declarative.

## Safety And Trust Model

This is a trust-focused local system utility. Prefer boring, explicit behavior over clever behavior.

Safety expectations:

- Do not delete files.
- Do not mutate project directories except through explicit optional backends approved by the user.
- Do not shell out through `sh -c` for path-dependent operations.
- Do not follow symlinks by default.
- Do not scan the entire filesystem by default.
- Do not silently ignore backend failures.
- Do not treat application bundles, package manager internals, or language runtime installs as obviously safe unless explicitly configured.
- Do not require network access at runtime.

The tool's job is to manage exclusion metadata, not to clean caches or decide what files are disposable beyond backup/index exclusion.

## Testing Expectations

Testing should cover the behavior that upstream `asimov` did not fully cover.

Important test areas include:

- Rule matching.
- Requirement evaluation, including AND requirements and OR evidence alternatives.
- Multiple matches in one run.
- Idempotence.
- Pruning matched directories.
- Skipping configured paths.
- Symlink behavior.
- Permission and unreadable-directory behavior where feasible.
- Paths with spaces and unusual but valid characters.
- Dry-run behavior.
- Backend command invocation boundaries.
- Per-path failure reporting and continued execution.

Use test doubles around the `tmutil` boundary rather than invoking real Time Machine operations in ordinary unit tests. Real macOS integration tests may be useful, but they should be opt-in and carefully documented.

Do not verify scanner or CLI behavior by running `tmignore` against real workspace roots such as `.`, `..`, `$HOME`, or `/` unless the user explicitly requests that exact command. Even dry-run scans traverse the filesystem, can expose private paths in logs, and can hit real permission boundaries. Use `tempfile` or other isolated fixtures for scanner and CLI verification. If a broad-root behavior needs coverage, reproduce it with a synthetic temporary directory tree.

## Naming

Use `tmignore` as the project/product name going forward. Avoid introducing public-facing references to `asimov-rs` except where unavoidable during repository transition.

When referring to upstream, call it `upstream asimov` or `stevegrunwell/asimov` to avoid confusing this project with the original.

## Non-Goals

Do not build a general cache cleaner.

Do not build a cross-platform backup abstraction unless the user explicitly changes scope.

Do not prioritize a Homebrew formula, installer script, or public package registry workflow before the Nix/nix-darwin use case.

Do not optimize for compatibility with upstream `asimov` output, flags, installation paths, or exact defaults unless those choices are independently correct.

Do not turn this file into an implementation plan. It is project context and operating guidance for future agents.
