# Contributing

English | [简体中文](CONTRIBUTING.zh-CN.md)

We welcome issue reports, documentation improvements, bug fixes, and feature contributions. To install the service, start with the [installation guide](https://git.neighbour.sg/neighbour/nomail-docs/src/branch/main/guide/install.md). For major feature, API, or schema changes, open an issue or discussion first to explain the goals and proposed approach.

## Development environment

- Rust 1.99+ and the system build tools required by your platform.
- `rustfmt` and Clippy; WASM checks also require `wasm32-unknown-unknown`.
- Node.js 22+ and `worker-build` when modifying Workers or running its integration tests.
- Docker is only needed to build and validate container images; native tests do not require an external database.

```bash
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown
```

See the [installation guide](https://git.neighbour.sg/neighbour/nomail-docs/src/branch/main/guide/install.md) for local setup and secret configuration. Do not use production credentials or production databases for development or testing.

## Where code belongs

```text
crates/nomail-core/       Platform signature verification, decryption, parsing, and basic content extraction
crates/nomail-app/        Open-source business logic, storage interfaces, receive use cases, and the single HTTP execution layer
crates/nomail-server/     Actix, SQLite, native runtime, and in-process SSE
crates/nomail-workers/    Workers, D1, WASM runtime, and Durable Objects
```

Dependency direction: `Server / Workers → App → Core`. Within App, the HTTP layer depends on business logic, not the other way around. See the [Core](crates/nomail-core/README.md), [App](crates/nomail-app/README.md), and [Workers](crates/nomail-workers/README.md) documentation for details.

- **Platform protocol changes:** Update verification, decryption, parsing, and tests in the corresponding platform directory in Core. Do not add account, database, system clock, or open-source inbox policy dependencies.
- **Business logic changes:** Implement them in App so both runtimes share them. Storage interfaces must express atomic operations; do not split refresh token consumption, ownership checks, or concurrent partial updates into unprotected CRUD operations.
- **New endpoints:** Define request/response DTOs centrally in App's `http/contract/` and associate routes in `http/contract/routes.rs`. Use `http/dispatch.rs` to convert business inputs/results and bind use cases. Business logic and storage must not use HTTP DTOs. Do not add `Operation` branches or per-endpoint handlers to either runtime.
- **Deployment changes:** Both runtimes only handle requests, responses, storage, execution capabilities, and real-time connections; do not reimplement business rules there.
- **New platform integrations:** Core protocol support and product availability are separate steps. Also check receive configuration, credential models, public APIs, and storage tests for both runtimes.

Preserve existing API behavior unless a change explicitly documents its incompatibilities. Historical PRDs, migration notes, and missing-endpoint analyses do not define the current contract; use the code, generated OpenAPI, and tests as the source of truth.

## Tests and checks

Run from the repository root:

```bash
cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --doc
cargo check --locked -p nomail-core --target wasm32-unknown-unknown
cargo check --locked -p nomail-app --target wasm32-unknown-unknown
cargo check --locked -p nomail-workers --target wasm32-unknown-unknown
cargo build --locked --release -p nomail-server --bins
```

Generate OpenAPI before compiling tests, including `cargo test` and checks such as Clippy or `cargo check --all-targets`: Server tests include the local JSON at compile time. Regenerate after changing contracts or switching branches; an existing file might be stale. Normal server builds and Docker builds do not need this file. Server integration tests use isolated temporary SQLite files. App architecture tests check dependency boundaries and prevent the two runtimes from maintaining their own endpoint dispatch logic.

For changes involving Workers, shared HTTP behavior, or business logic, also run the local D1/DO tests:

```bash
cargo install worker-build --locked --version 0.8.7
cd crates/nomail-workers
npm ci
npm test
```

These tests use temporary local D1 and Durable Objects instances, require no Cloudflare account, and do not connect to remote databases. They use port `8791`. Ensure `worker-build` is in `PATH` and the port is available.

## Updating API documentation

`docs/swagger.json` is generated from the actual DTOs and route contracts. It can remain on disk for local use but is ignored by Git: do not edit it manually or commit it. Its `info.version` comes from the workspace package version. Run from the repository root:

```bash
cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json
cargo test --locked -p nomail-app --features openapi --lib
```

Commit generator/contract changes and their tests, not the generated JSON. Tests compare the local generated file with current contracts and check routes, schemas, and references; Workers tests also validate actual responses against it. CI always generates and verifies a fresh file, then shares that exact artifact with its test jobs and the release workflow. Swagger generation or validation failure blocks all release builds and publishing. Releases attach `swagger.json` separately and include it in `SHA256SUMS`; it is not added to binary archives or Docker images. Use the attachment for the version you run, not necessarily the latest release. Older releases may not have it; generate locally when needed.

Update `nomail-docs/guide/install.md` for changes to user procedures, and maintain API usage documentation in `nomail-docs/reference/`. Update this guide when contribution procedures change; keep the README limited to the project introduction and documentation links. See the [nomail-docs](https://git.neighbour.sg/neighbour/nomail-docs) documentation repository.

## Commits and reviews

### Commit conventions

Use the Conventional Commits format:

```text
<type>[optional scope][optional !]: <short description in English>

[optional body]

[optional footer]
```

| Type | Purpose |
|---|---|
| `feat` | New features |
| `fix` | Bug fixes |
| `refactor` | Refactoring without behavior changes |
| `perf` | Performance improvements |
| `docs` | Documentation |
| `test` | Tests |
| `build` | Build, packaging, and dependencies |
| `ci` | CI configuration, such as GitHub Actions |
| `chore` | Maintenance not covered by the types above |
| `revert` | Reverting an existing commit |

- Write the subject in English, use the imperative mood, and omit the trailing period. Aim for no more than 72 characters.
- The scope is optional; use `core`, `app`, `server`, or `workers` for changes limited to one crate.
- Each commit should address one complete concern. Separate refactoring from behavior changes where possible, and keep each batch buildable and verifiable.
- Commit unrelated changes separately. Avoid vague subjects such as `update` or `fix bug`.
- For complex changes, explain the rationale, impact, and necessary verification details in the body rather than merely repeating what changed.
- For breaking changes, add `!` after the type or scope and include a `BREAKING CHANGE:` footer describing the impact and migration steps.
- When reverting a commit, include its SHA in the body.

Examples:

```text
feat(core): support a new webhook provider
fix(server): handle invalid forwarding headers
ci: publish release binaries
docs: remove obsolete migration guide
```

Breaking change example:

```text
feat(app)!: change the pagination response format

BREAKING CHANGE: Replace the total field with hasMore. Clients must update
their pagination handling before upgrading.
```

### Pull request and review requirements

- Describe the motivation, main changes, verification commands, and impact on APIs, databases, deployment, and compatibility.
- Bug fixes should include regression tests that reproduce the original issue. Verify both runtimes for changes affecting both deployment options.
- Do not commit `.env`, `.dev.vars`, tokens, real webhook payloads, databases, `target/`, `node_modules/`, or build artifacts. Review the diff before committing and sanitize example data.
- Do not include real secrets in public reports of credential or security issues. Coordinate privately with the repository maintainers first.

## Version maintenance

- The project version is defined in `[workspace.package].version` in the root `Cargo.toml`; all four crates inherit it.
- Record user-facing changes under `Unreleased` in `CHANGELOG.md`.
- Before a release, update the version, run `cargo check` to synchronize `Cargo.lock`, and move pending entries under the corresponding version and actual release date while retaining the `Unreleased` section. Commit after checks pass.
- GitHub Release notes come from the `CHANGELOG.md` entry that exactly matches the tag (for example, `v0.2.0` matches `## [0.2.0] - YYYY-MM-DD`). Do not use GitHub-generated notes or `Unreleased`. Prereleases require their own full-version entries. Release checks fail if an entry is missing, duplicated, or empty. Rerunning an existing release updates assets only and does not overwrite its original notes.
- Push a tag matching the project version according to the [Git tag conventions](docs/tagging.md) to automatically publish images and a GitHub Release. Do not publish older stable versions retroactively, as this could overwrite `latest`.
- CI for regular pushes and pull requests first generates and verifies OpenAPI, then runs formatting checks, Rust tests for the default workspace members, workflow/release-note script tests, and a separate Workers WASM/local D1/DO job using the verified artifact. It does not replace the full checks above. Run the script tests locally with `python3 -m unittest discover -s .github/scripts -p 'test_*.py'`.

The check workflow is `.github/workflows/ci.yml`. The release workflow is `.github/workflows/release.yml`; it is triggered only by version tags and reuses the checks before publishing.
