# RULES

Coding rules for this project. Every contributor (human or AI) must follow these.

## File Structure

- **Max 300 lines per file**. If a file grows beyond, split by functionality.
- Unit tests live in `tests/` directory at crate root, one file per module tested.
- Integration tests live in `tests/integration/` when applicable.
- Tests are **never** mixed with production code (no `#[cfg(test)]` inline modules).

## Logging

- Every public function must have structured logging (`tracing::info!`, `tracing::debug!`).
- Use per-functionality targets: `tracing::info!(target: "nexum::<module>::<feature>", ...)`.
- Entry/exit logging for complex operations: `ENTER` / `EXIT` with timing.
- Import logging utilities from `nxm-shared` (never roll your own).

## Error Handling

- Never use `unwrap()` or `expect()` in production code. Only allowed in tests.
- Use `anyhow::Result` for application-level errors.
- Use `thiserror` for library-level errors with typed variants.
- Always provide context: `.context("what was happening when this failed")`.

## Naming & Style

- Language: English for code, comments, and documentation.
- Italian allowed in: design docs, brainstorm notes, commit messages.
- Rust naming: `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE` for constants.
- Modules named after functionality, not implementation detail.

## Dependencies

- Shared utilities (logging, config, errors) come from `nxm-shared`. Do not duplicate.
- Pin exact versions in Cargo.toml for external crates.
- Prefer workspace dependencies declared in root `Cargo.toml`.

## Testing

- Every new feature must have unit tests before merge.
- Integration tests for cross-module or cross-crate interactions.
- Test file naming: `tests/<module_name>.rs` or `tests/integration/<feature>.rs`.
- Use `#[ignore]` for tests requiring hardware (GPU, ANE) with a comment explaining why.

## Documentation

- Update `CONTEXT.md` at the end of every coding session.
- Update `CHANGELOG.md` with every meaningful change (not typos/formatting).
- Public APIs must have `///` doc comments with at least one example.

## Commits

- Follow Conventional Commits: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`, `perf:`, `chore:`.
- One logical change per commit. Don't mix features with refactors.


## Project Documents

- **Plans, designs, loops, and handoffs** go in `nxm-ai/nxm-projects` (not here).
- This repo keeps only: RULES.md, CONTEXT.md, STATUS.md, CHANGELOG.md.
- When starting a multi-session effort, create a plan in `nxm-projects/plans/active/`.
- When ending a long session, create a handoff in `nxm-projects/handoffs/active/`.
- When running an iterative loop, document it in `nxm-projects/loops/active/`.
