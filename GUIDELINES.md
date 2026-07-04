# Code Guidelines

## General

Follow general code best practices:

- <IMPORTANT>: Always aim for the most correct change or fix, never the shortest or simplest. This is extremely important. The codebase is in an alpha state, so there is no "legacy" or "old behavior" to maintain whatsoever. The best design/architecture is always the most important priority.
- Use descriptive and well-chosen names for variables, functions, and classes.
- Avoid redundant duplication: if a string or a magic number is being duplicated multiple times, extract it to a single shared place.
- Avoid repeating yourself, such as with hard-coded strings or magic numbers; use constants or functions instead.
- Each function should do one 'job' and do it well.
- Avoid reinventing the wheel when a well-known library or tool can accomplish the task effectively.
- Compatibility is NOT important. The project is in pre-alpha, so this isn't yet a concern. Always architect for the optimal design, don't worry about keeping things compatible.
- Always plan the most correct plan/fix, not necessarily the smallest or most convenient plan/fix.

## Git

- Use concise commit messages in the existing `scope: imperative summary` style, such as `server: add router integration tests` or `core: add app config env parsing`.
- Prefer scopes that match the touched area or crate, such as `rust`, `server`, `db`, `web`, or `core`.

## Rust

Be eager about dependency usage rather than reinventing the wheel. Always search dependencies for the latest versions using cargo before recommending.

Structure imports:
use std::<whatever>
<new line>
use <dependency>::<whatever>
<new line>
use <crate name>::<whatever>

## Database

- The project is pre-alpha. Destructive schema and migration rewrites are acceptable when they produce the most correct design. Do not preserve old database compatibility unless explicitly asked.
- If data needs to be stored, prefer sqlx with sqlite.

## Nix

Use flake-parts.
