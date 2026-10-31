# 贡献指南

欢迎提交问题报告、文档改进、修复和功能贡献。安装服务请先阅读[安装指南](docs/install.md)；较大的功能、接口或 Schema 变更，请先通过 Issue/讨论说明目标和方案。

## 开发环境

- Rust 1.99+，以及平台所需的系统编译工具。
- `rustfmt`、Clippy；WASM 检查需要 `wasm32-unknown-unknown`。
- 修改 Workers 或运行其集成测试时，需要 Node.js 22+ 和 `worker-build`。
- Docker 只在构建、验证容器镜像时需要；原生测试无需外部数据库。

```bash
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown
```

本地运行方法和密钥配置见[安装指南](docs/install.md)。不要使用生产凭据或生产数据库进行开发和测试。

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

接口变更应同时提交生成文件；CI 会比较文档快照，Workers 测试会校验实际响应的 Schema。用户操作变化更新 `docs/install.md`，贡献流程变化更新本文件，README 仅保留项目简介和文档入口。

## 提交与评审

- 一个提交解决一个完整问题；重构与行为变化尽量分开，每一批保持可编译、可验证。
- 提交信息可使用 `feat:`、`fix:`、`refactor:`、`test:`、`docs:`、`build:` 前缀。
- PR 说明动机、主要变化、验证命令，以及对接口、数据库、部署和兼容性的影响。
- 修复应包含能复现原问题的回归测试；影响两种部署的改动应验证两端。
- 不提交 `.env`、`.dev.vars`、Token、真实 Webhook 原文、数据库、`target/`、`node_modules/` 或构建产物。提交前检查 diff，样例数据应脱敏。
- 凭据或安全问题不要附带真实密钥公开报告；先通过仓库维护者的私下联系方式协调处理。

## CI 与镜像发布（维护者）

工作流位于 `.gitea/workflows/ci.yml`：

- `test` 执行格式、Clippy、原生测试、文档/WASM 检查和发布构建。
- `integration` 执行本地 Workers D1/DO 测试，可与 `test` 并行。
- `docker` 等待前两项成功，仅在 `main` 推送、`v*` 标签或手动触发时发布镜像；当前只构建 `linux/amd64`，Runner 需支持对应平台。

GHCR 发布配置：

| 设置 | 用途 |
|---|---|
| Secret `GHCR_USERNAME` | 有 Package 发布权限的 GitHub 账号，不是 Gitea 用户名 |
| Secret `GHCR_TOKEN` | 例如 classic PAT，需具备 `write:packages` 及相应 Package 权限 |
| Variable `GHCR_NAMESPACE` | 可选，覆盖默认账号命名空间；组织发布需确认授权 |

镜像地址为 `ghcr.io/<namespace>/nomail-api`，`main` 发布 `latest`，版本标签发布语义化标签，另有 SHA 标签。`buildcache` 仅用于构建缓存，不是运行镜像。远程层缓存不等于导出了 Dockerfile 中的 Cargo cache mount。

确认 GHCR Package 的实际可见性；工作流不会将已有公开 Package 自动改为私有。组织 SSO 可能需要额外授权。部署端使用独立的只读凭据，不复用发布 Token；使用方法见[安装指南](docs/install.md)。

## 贡献授权

项目采用 **AGPL-3.0-only**，完整条款见 [LICENSE](LICENSE)。除另有书面约定外，提交供本项目合并的贡献应按相同许可提供，并确认你拥有提交该贡献所需的权利及必要的雇主授权。

贡献者保留其贡献的版权。提交 PR 不表示转让版权，也不额外授予维护者将贡献用于闭源产品或其他许可下再授权的权利；本项目目前未采用 CLA。

引入第三方代码时，请说明来源及许可，保留必要声明，并确认其与项目许可兼容。
