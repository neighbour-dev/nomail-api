# NoMail API

单用户、自托管通知收件箱服务端，使用 **Rust + Actix Web + SeaORM + PostgreSQL**。

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

```bash
docker build -t nomail-api:rust .
docker run --rm --env-file .env -p 3000:3000 nomail-api:rust
```

现有 `compose.yml` 可继续使用，数据库由外部提供；容器中的 `DB_HOST` 不能指向容器自身的 localhost。
镜像附带 `reset-account`、`gen-secret-key`，可通过 `docker exec` 调用。

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
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release --bins
```

集成测试必须使用**独立的测试数据库**（需要创建 Schema 权限）。每次测试创建随机 Schema，成功后删除；失败时可留下 Schema 用于排查。
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
