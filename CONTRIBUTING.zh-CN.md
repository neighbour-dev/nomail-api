# 贡献指南

[English](CONTRIBUTING.md) | 简体中文

欢迎提交问题报告、文档改进、修复和功能贡献。安装服务请先阅读[安装指南](https://git.neighbour.sg/neighbour/nomail-docs/src/branch/main/guide/install.md)；较大的功能、接口或 Schema 变更，请先通过 Issue/讨论说明目标和方案。

## 开发环境

- Rust 1.99+，以及平台所需的系统编译工具。
- `rustfmt`、Clippy；WASM 检查需要 `wasm32-unknown-unknown`。
- 修改 Workers 或运行其集成测试时，需要 Node.js 22+ 和 `worker-build`。
- Docker 只在构建、验证容器镜像时需要；原生测试无需外部数据库。

```bash
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown
```

本地运行方法和密钥配置见[安装指南](https://git.neighbour.sg/neighbour/nomail-docs/src/branch/main/guide/install.md)。不要使用生产凭据或生产数据库进行开发和测试。

## 代码放在哪里

```text
crates/nomail-core/       平台验签、解密、解析和基础内容提取
crates/nomail-app/        开源业务、存储接口、接收用例与唯一 HTTP 执行
crates/nomail-server/     Actix、SQLite、原生运行时与进程内 SSE
crates/nomail-workers/    Workers、D1、WASM 运行时与 Durable Objects
```

依赖方向：`Server / Workers → App → Core`。App 内部 HTTP 层依赖业务，业务不反向引用 HTTP。详细说明见 [Core](crates/nomail-core/README.md)、[App](crates/nomail-app/README.md) 和 [Workers](crates/nomail-workers/README.md)。

- **修改平台协议：**在 Core 对应平台目录修改验签、解密、解析与测试；不要加入账号、数据库、系统时钟或开源版收件箱策略。
- **修改业务：**在 App 实现，两端共用。存储接口表达原子操作，不要把刷新令牌消费、所有权检查或并发局部更新拆成无保护的 CRUD。
- **新增接口：**在 App `http/contract/` 集中定义请求/响应 DTO，并在 `http/contract/routes.rs` 关联路由；`http/dispatch.rs` 转换业务输入/结果并绑定用例。业务和存储不使用 HTTP DTO，不向两端添加 `Operation` 分支或逐接口 Handler。
- **修改部署：**两端仅处理请求、响应、存储、执行能力与实时连接，不重新实现业务规则。
- **新增接入平台：**Core 协议支持与产品开放是两个步骤，还需核对接收配置、凭据模型、公开接口和两端存储测试。

保持已有 API 行为，除非变更明确说明不兼容性。历史 PRD、迁移记录和缺失接口分析不代表当前契约；以代码、生成的 OpenAPI 和测试为准。

## 测试与检查

在仓库根目录执行：

```bash
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --doc
cargo check --locked -p nomail-core --target wasm32-unknown-unknown
cargo check --locked -p nomail-app --target wasm32-unknown-unknown
cargo check --locked -p nomail-workers --target wasm32-unknown-unknown
cargo build --locked --release -p nomail-server --bins
```

Server 集成测试使用独立的临时 SQLite 文件。App 的架构测试检查依赖边界，并防止两端重新维护接口分发。

涉及 Workers、共享 HTTP 或业务行为时，还要运行本地 D1/DO 测试：

```bash
cargo install worker-build --locked --version 0.8.7
cd crates/nomail-workers
npm ci
npm test
```

该测试使用临时本地 D1 和 Durable Objects，不需要 Cloudflare 账号、不连接远程数据库，使用端口 `8791`。确保 `worker-build` 在 `PATH` 中且端口未被占用。

## 更新接口文档

`docs/swagger.json` 由真实 DTO 和路由契约生成，不手工编辑。在仓库根目录执行：

```bash
cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json
cargo test --locked -p nomail-app --features openapi --lib
```

接口变更应同时提交生成文件并核对文档快照，Workers 测试会校验实际响应的 Schema。用户操作变化更新 `nomail-docs/guide/install.md`，接口使用说明在 `nomail-docs/reference/` 中维护；贡献流程变化更新本文件，README 仅保留项目简介和文档入口。文档仓库见 [nomail-docs](https://git.neighbour.sg/neighbour/nomail-docs)。

## 提交与评审

### 提交规范

提交信息采用 Conventional Commits 格式：

```text
<type>[可选 scope][可选 !]: <英文简短描述>

[可选正文]

[可选脚注]
```

| 类型 | 用途 |
|---|---|
| `feat` | 新功能 |
| `fix` | 修复问题 |
| `refactor` | 不改变功能的重构 |
| `perf` | 性能优化 |
| `docs` | 文档 |
| `test` | 测试 |
| `build` | 构建、打包与依赖 |
| `ci` | GitHub Actions 等 CI 配置 |
| `chore` | 不属于以上类型的维护工作 |
| `revert` | 撤销已有提交 |

- 标题使用英文、动词原形，不加句号，建议不超过 72 字符。
- scope 可省略；涉及单个 crate 时可使用 `core`、`app`、`server`、`workers`。
- 一个提交解决一个完整问题；重构与行为变化尽量分开，每一批保持可编译、可验证。
- 不相关的修改分开提交，避免使用 `update`、`fix bug` 等无法说明目的的标题。
- 复杂变更在正文说明原因、影响和必要的验证信息，不只重复修改内容。
- 不兼容变更在类型或 scope 后加 `!`，并在脚注中使用 `BREAKING CHANGE:` 说明影响和迁移方式。
- 撤销提交时在正文注明被撤销的 commit SHA。

示例：

```text
feat(core): support a new webhook provider
fix(server): handle invalid forwarding headers
ci: publish release binaries
docs: remove obsolete migration guide
```

不兼容变更示例：

```text
feat(app)!: change the pagination response format

BREAKING CHANGE: Replace the total field with hasMore. Clients must update
their pagination handling before upgrading.
```

### PR 与评审要求

- PR 说明动机、主要变化、验证命令，以及对接口、数据库、部署和兼容性的影响。
- 修复应包含能复现原问题的回归测试；影响两种部署的改动应验证两端。
- 不提交 `.env`、`.dev.vars`、Token、真实 Webhook 原文、数据库、`target/`、`node_modules/` 或构建产物。提交前检查 diff，样例数据应脱敏。
- 凭据或安全问题不要附带真实密钥公开报告；先通过仓库维护者的私下联系方式协调处理。

## 版本维护

- 项目版本统一定义在根目录 `Cargo.toml` 的 `[workspace.package].version`，四个 crate 继承同一版本。
- 面向用户的变更记录在 `CHANGELOG.md` 的 `Unreleased` 中。
- 发布前更新版本，运行 `cargo check` 同步 `Cargo.lock`，将待发布条目整理到对应版本和实际发布日期下，保留 `Unreleased` 区域；检查通过后提交。
- GitHub Release 正文取自 `CHANGELOG.md` 中与标签精确对应的版本条目（例如 `v0.2.0` 对应 `## [0.2.0] - 日期`），不使用 GitHub 自动生成说明，也不使用 `Unreleased`。预发布需单独记录完整版本号；条目缺失、重复或为空时，发布检查失败。已有 Release 重跑仅更新附件，不覆盖原正文。
- 按 [Git Tag 规范](docs/tagging.md) 推送与项目版本一致的标签，会自动发布镜像和 GitHub Release；不要补发旧稳定版本，以免覆盖 `latest`。
- 普通 push / PR 的 CI 执行格式检查、默认成员的 Rust 测试、发布说明提取脚本测试及独立的 Workers WASM/OpenAPI/本地 D1/DO 检查，不能替代上述完整检查。脚本测试可在本地运行：`python3 -m unittest discover -s .github/scripts -p 'test_*.py'`。

检查工作流为 `.github/workflows/ci.yml`；发布工作流为 `.github/workflows/release.yml`，仅由版本标签触发，复用检查通过后再发布。
