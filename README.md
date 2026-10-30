# NoMail API

单用户、自托管通知收件箱服务端，使用 **Rust + Actix Web + SeaORM + PostgreSQL**。

## Workspace 与共享内核

- 根 crate `nomail-api`：保留现有服务端入口、HTTP API、数据库、账号、消息和广播实现。
- `crates/nomail-core`：独立、无运行时依赖的 Webhook 认证/解密/解析与内置消息映射，支持编译到 WASM。详见 [Core 使用说明](crates/nomail-core/README.md)。

业务请求输出 `PlatformEvent`，再通过 `event.to_message()` 生成统一收件箱消息；验证/Ping 返回独立的控制结果。服务端 `src/webhook/parser.rs` 仅承担请求头、配置和错误适配，不再维护一套解析/映射规则。

`PlatformEvent` 分为公共 `metadata`、平台专有 `data` 和可读原始内容 `raw`。`raw` 是解密/解码后、未经消息映射的业务内容，不是验签用的 HTTP 字节。新消息的 `rawPayload` 存储该可读表示，`rawContentType` 描述它的实际格式（例如 GitHub 表单解包后为 JSON）；旧消息不自动重写。当前仍存储消息与可读原始内容，尚未新增独立事件表或完整事件归档。

Core 覆盖 Generic、GitHub、GitLab、Stripe、Slack、飞书、Telegram、Shopify 的协议信封；**当前 HTTP API 仍仅开放 Generic / GitHub / 飞书**。其他平台需要先补充服务端配置与凭据模型。尚未创建 Workers 应用，也未实现 D1、Durable Objects 或 Queues 适配。

**飞书兼容性注意：**旧服务端只有一个 `secret` 字段，当前适配将它同时作为验证 Token 和 Encrypt Key，普通事件必须提供签名，不再因签名缺失回退为固定 Token；带时间戳签名会检查前后 300 秒。只有 Token 的明文普通事件端点，以及 Token/Encrypt Key 不同的加密验证端点，需要后续扩展服务端配置后才能正确接入。Core 本身已支持独立配置两种凭据。GitHub Ping 作为控制请求处理，不再生成收件箱消息；GitHub 去重只使用投递 ID，不回退资源 ID。

## 启动

需要 Rust 1.99+、PostgreSQL 及可用的数据库账号。

```bash
cp .env.example .env
cargo run --bin gen-secret-key
```

将生成的密钥写入 `.env`，配置数据库连接、`JWT_SECRET` 和初始账号，然后运行：

```bash
cargo run --bin nomail-api
```

服务默认监听 `0.0.0.0:3000`。启动时应用版本化数据库迁移；数据库须预先创建。
连接支持 `DATABASE_URL`（优先）或 `DB_HOST/DB_PORT/DB_USERNAME/DB_PASSWORD/DB_DATABASE`。

首次启动仅在账号表为空时创建账号。密码至少 8 个字符，包含英文字母和数字，最多 72 字节。
创建后移除 `INITIAL_PASSWORD`。公开注册不提供。

## 账号维护

`GET /api/auth/account` 获取账号；`PATCH /api/auth/account` 修改账号，均需要 Bearer Token：

```json
{
  "currentPassword": "current-password1",
  "username": "new-username",
  "newPassword": "new-password1"
}
```

`username`、`newPassword` 至少提供一项。修改密码会撤销全部登录会话。
忘记密码时在服务器本地执行（`--username` 可省略）：

```bash
cargo run --bin reset-account -- --username admin --password new-password1
```

## Docker

镜像使用 Alpine Linux；构建阶段同样使用 Alpine 的 musl 工具链，运行阶段保留 CA 证书、时区数据和非 root 用户。健康检查使用内置 `wget`，无需安装 curl。

```bash
docker build -t nomail-api:rust .
docker run --rm --env-file .env -p 3000:3000 nomail-api:rust
```

现有 `compose.yml` 可继续使用，数据库由外部提供；容器中的 `DB_HOST` 不能指向容器自身的 localhost。
镜像附带 `reset-account`、`gen-secret-key`，可通过 `docker exec` 调用。

### GHCR 私有镜像

Gitea Actions 的 `.gitea/workflows/ci.yml` 在普通检查和数据库集成测试均成功后，仅构建 `linux/amd64` 镜像并推送到（Runner 需为 AMD64，无需 QEMU）：

```text
ghcr.io/<github-account-or-organization>/nomail-api:<tag>
```

在 **Gitea 仓库 Settings → Actions → Secrets** 中设置：

| Secret | 内容 |
|---|---|
| `GHCR_USERNAME` | 用于登录 GHCR 的 GitHub 用户名，不是 Gitea 用户名 |
| `GHCR_TOKEN` | GitHub Personal Access Token **classic**，至少包含 `write:packages` |

默认推送到 `GHCR_USERNAME` 对应的个人命名空间。如果要推送到 GitHub 组织，在 **Actions → Variables** 中设置 `GHCR_NAMESPACE`，并确保该用户有权向组织发布 Package。CI 会将命名空间转为小写。
Gitea 的内置 Token 不能用于登录 GHCR；组织要求 SSO 时，还需为 GitHub Token 授权。

推送 `main` 发布 `latest` 和 SHA 标签；推送 `v*` 版本标签发布对应的语义化版本标签和 SHA 标签。
构建层缓存保存到同一 GHCR Package 的 `buildcache` 标签，供后续 CI 构建复用；该标签不是可部署的应用镜像。首次构建没有远程缓存是正常情况。
工作流启用 `mode=min` 构建来源证明（provenance），不再禁用相关元数据。远程缓存保存构建层，但不会导出 Dockerfile 中 `RUN --mount=type=cache` 的 Cargo 缓存目录。
**GHCR 首次创建的 Package 默认私有**。首次发布后进入 GitHub 账号/组织的 Packages 页面确认可见性为 **Private**；若同名 Package 原已公开，本工作流不会自动将其改为私有。
源码无需迁移到 GitHub，也无需公开。

部署服务器使用单独的 GitHub Token（classic，仅需 `read:packages`，且所属用户有镜像读取权限）：

```bash
docker login ghcr.io --username YOUR_GITHUB_USERNAME
# 在密码提示中输入只读 Token，不是 GitHub 登录密码。
```

在 `compose.yml` 中直接填写镜像地址，将 `your-github-username` 替换为 CI 使用的小写 GitHub 用户名或组织名：

```yaml
image: ghcr.io/your-github-username/nomail-api:latest
```

```bash
docker compose pull
docker compose up -d
```

镜像标签在 `compose.yml` 中直接指定为 `latest`；如需固定版本，修改该文件的 `image` 标签即可。

不要把 Token 写入 `.env`、Compose 或版本库。生产服务器建议配置 Docker credential helper 安全保存登录凭据。

## 代码导航

```text
src/main.rs          启动服务，共享连接池和实时事件 Hub
src/app.rs           /api 路由入口、HTTP 配置、健康检查
src/auth/            登录、刷新、账号、设备；mod.rs 定义鉴权提取器
src/webhook/         配置、原始请求验签、解析和消息入库
src/message/         消息 DTO、查询、CRUD、批量操作及 SSE
src/push.rs          加密 Push Token 的注册、更新和删除
src/realtime.rs      按用户隔离的进程内事件广播
src/entity/          SeaORM 表映射，不直接作为 API 响应
src/crypto.rs        AES-GCM、bcrypt、Token 哈希
src/migration.rs     版本化迁移入口
migration/           PostgreSQL Schema 基线
tests/api.rs         PostgreSQL HTTP 集成测试
```

例如阅读消息列表：`message/mod.rs` → `handler::list` → `repository::filtered` → `entity/message.rs`。
简单接口不强制增加 Service 层。

## 测试与构建

```bash
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
cargo test --locked -p nomail-core --doc
cargo check --locked -p nomail-core --target wasm32-unknown-unknown
cargo build --locked --release --bins
```

CI 统一放在 `.gitea/workflows/ci.yml`，按以下依赖关系运行：

- `test`：格式检查、Clippy、默认测试和发布构建，不依赖数据库。
- `integration`：自动启动临时 PostgreSQL 17，验证数据库连接后运行集成测试。任务明确运行在容器内，通过服务名 `postgres:5432` 访问数据库，不使用 `localhost`，也不占用宿主机的 5432 端口。
- `docker`：通过 `needs: [test, integration]` 等待上述两个 Job 均成功后才构建、推送镜像。仅 `main` 分支推送、`v*` 标签推送或手动触发时执行；PR 和其他分支推送只运行检查和测试。

`test` 和 `integration` 可以并行运行；任意一项失败都会阻止镜像发布。原独立的 `docker.yml` 已合并移除，避免镜像发布绕过测试。

Gitea Runner 必须能够访问 Docker，并允许任务容器与服务容器在同一网络通信。此 Job 使用 `node:22-bookworm` 以支持 Checkout Action，并安装 Rust 工具链和 PostgreSQL 客户端。数据库使用独立的测试凭据、不配置持久化卷，任务结束后由 Runner 清理服务容器，无需提前部署 PostgreSQL，也不会连接生产数据库。

本地执行 `cargo test --locked --all-targets` 仍会跳过数据库集成测试。需要手动运行时，必须使用**独立的测试数据库**（需要创建 Schema 权限）。每次测试创建随机 Schema，成功后删除；失败时可留下 Schema 用于排查。
不要指向生产数据库。

```bash
TEST_DATABASE_URL=postgres://postgres:password@localhost:5432/nomail_test cargo test --locked --test api -- --ignored
```

## 升级与能力边界

升级前备份数据库，保留原 `JWT_SECRET`、`JWT_ISSUER`、`SECRET_ENCRYPTION_KEY`。
基线迁移通过 `CREATE TABLE IF NOT EXISTS` 接管旧表，不重建或清空数据；请先在生产备份上验证，见 [迁移说明](docs/rust-migration.md)。

已支持 27 个业务路由，包括 GitHub / 飞书 / Generic Webhook、幂等入库、消息管理、设备会话与 SSE。
**APNs/FCM 实际发送、持久化推送任务及自动过期清理尚未实现**，Push Endpoint 注册不代表已能发送手机通知。
SSE 仅进程内广播，慢客户端可能丢事件，无重放；多实例部署需要外部事件总线。

`docs/swagger.json` 可查阅接口定义；旧 PRD 和缺失接口分析是历史规划，不应替代当前路由及测试。
