# NoMail API

单用户、自托管通知收件箱，提供 **Rust + Actix Web + SeaORM + SQLite** 服务器部署，以及 **Cloudflare 官方 worker-rs + D1 + Durable Objects** 部署。

## Workspace 与共享内核

- 根目录是纯 Cargo Workspace，不再包含应用 crate。
- `crates/nomail-app`：开源版账号、接收配置、收件箱、完整接收用例及唯一 HTTP 契约，供两种部署共享。见 [App 说明](crates/nomail-app/README.md)。
- `crates/nomail-server`：Actix HTTP、SQLite、账号、消息与进程内 SSE；启动二进制仍叫 `nomail-api`。
- `crates/nomail-workers`：官方 `worker` SDK 的 HTTP、D1 和 Durable Objects SSE。见 [Workers 部署说明](crates/nomail-workers/README.md)。
- `crates/nomail-core`：独立、无运行时依赖的 Webhook 认证/解密/解析与基础内容提取，支持编译到 WASM。可供未来 SaaS 单独依赖，不包含开源版账号、存储策略或 HTTP API。详见 [Core 使用说明](crates/nomail-core/README.md)。

依赖方向为 `Server / Workers → App → Core`；App 内部 `http` 依赖业务模块，业务不反向引用 HTTP。业务请求解析为 `PlatformEvent`，`event.to_message_content()` 仅提取标题、正文、来源、等级和链接等通用字段，不替代平台结构。App 的 `IngestionService` 统一查配置、解密凭据、调用协议、处理控制请求并幂等入箱。验证/Ping 单独响应，不入库。两端只负责请求、存储、运行时和实时连接适配。

`PlatformEvent` 分为公共 `metadata`、平台专有 `data` 和可读原始内容 `raw`。`raw` 是解密/解码后、未经消息映射的业务内容，不是验签用的 HTTP 字节。单张 `notify_messages` 保存平台快照（provider、类型、版本、带作用域的完整去重键、platform_data JSON）、通用内容、可读原文及已读/置顶/接收时间/过期时间等 NoMail 字段，不新增事件表。

列表查询不加载平台 JSON/raw，只返回预览字段，**每条都有 level**；详情增加 `provider`、`metadata`、`platformData` 供平台专属组件展示，底部原文通过 `/api/messages/{id}/raw` 获取。GitHub Issue/评论/PR/Workflow Run 的结构化信息及飞书发送者、聊天信息、嵌套 content JSON 都会保存。未建模字段保留在原文。修改配置或已读/置顶状态不重写历史平台内容。

`level` 是必填的关注优先级：`low < normal < high < urgent`，通过 `priority_rank()` / `is_at_least()` 比较，不按字符串排序。Generic JSON 的 `level` 或纯文本的 `X-NoMail-Level` 可提供优先级；缺失、未知标签默认 normal。success/failure 等结果不再作为优先级别名，GitHub/Stripe 等平台尚无明确优先级映射规则时统一默认 normal，原结果保留在平台数据中。通知与删除策略独立于优先级：当前所有等级均使用 3 天默认保留期，不新增规则引擎。API 的 `createdAt` 表示 NoMail 接收时间（App 中为 `received_at`），不是平台事件发生时间。

本轮直接调整 SQLite/D1 初始 Schema，不转换已有数据库；已有本地开发数据库不会被程序自动清空。搜索行为不变，仍不包含通知推送。

Core 覆盖 Generic、GitHub、GitLab、Stripe、Slack、飞书、Telegram、Shopify 的协议信封；**两种部署的 HTTP API 仅开放 Generic / GitHub / 飞书**。其他平台需要先补充配置与凭据模型。账号、密码规则、凭据存储加密、分页、接收配置校验和收件箱规范化均属于 App，不进入协议 Core。

**飞书兼容性注意：**旧服务端只有一个 `secret` 字段，当前适配将它同时作为验证 Token 和 Encrypt Key，普通事件必须提供签名，不再因签名缺失回退为固定 Token；带时间戳签名会检查前后 300 秒。只有 Token 的明文普通事件端点，以及 Token/Encrypt Key 不同的加密验证端点，需要后续扩展服务端配置后才能正确接入。Core 本身已支持独立配置两种凭据。GitHub Ping 作为控制请求处理，不再生成收件箱消息；GitHub 去重只使用投递 ID，不回退资源 ID。

## HTTP 接口契约

`crates/nomail-app/src/http/routes.rs` 是接口唯一清单：路径、方法、Operation、鉴权要求、请求体/响应种类及请求/查询/响应 DTO 在同一处关联。Server 和 Workers 一次接入共享路由解析器，穷尽匹配 Operation 绑定处理函数，不各自注册路径或维护“已知接口”白名单。新增操作遗漏任一端绑定会编译失败；`success::<Endpoint>()` 检查响应数据类型与 App 契约一致。

公共 `ApiResponse<T>`、错误映射、JSON/查询参数解析、4 MiB 请求体限制、尾斜杠和路径参数解码规则由 App HTTP 层定义。先匹配接口，再进行 Bearer 鉴权，随后解析输入；未知路径/不支持的方法返回 404，路径参数仅解码一次。普通 JSON 接口要求 application/json 或 application/*+json。SSE、可读原文、Webhook 验证/Ping 保持原协议，不强行套 JSON 包装，Webhook Body 原字节用于验签。

SSE 的 ready/message.created/auth.expired 名称、数据 DTO、帧编码和心跳统一在 `crates/nomail-app/src/http/sse.rs`；业务只产生 `MessageAccepted` 事件，由 HTTP 层转换为 SSE，两端负责连接与广播。配置业务和存储只返回无 URL 的 `ConfigView`，HTTP 层构造带 URL 的响应。OpenAPI 的 `x-sse-events` 描述逻辑事件结构，Workers 测试校验真实事件帧及订阅容量错误的公共 JSON 格式。

`docs/swagger.json` 是自动生成的 **OpenAPI 3.1**，包含平台详情的 Rust 类型 Schema；不要手工修改：

```bash
cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json
```

CI 检查文档快照与契约一致，Wrangler 测试依据生成文档校验实际 JSON 响应。`openapi` 是仅用于文档/测试的可选 feature，普通 Server/Workers 构建不链接 Schema 生成器。

## 启动服务

需要 Rust 1.99+，无需部署数据库服务。

```bash
cp .env.example .env
cargo run --bin gen-secret-key
```

将生成的密钥写入 `.env`，配置数据库连接、`JWT_SECRET` 和初始账号，然后运行：

```bash
cargo run --bin nomail-api
```

服务默认监听 `0.0.0.0:3000`。`DATABASE_PATH` 默认 `./data/nomail.db`；启动时自动创建目录、数据库与表。SQLite 使用 WAL、外键检查、写入等待和完整同步，小连接池适合单用户负载。

开源版不再支持 PostgreSQL，不提供旧数据库兼容或数据迁移。PostgreSQL 留给未来的私有 SaaS。

首次启动仅在账号表为空时创建账号。密码至少 8 个字符，包含英文字母和数字，最多 72 字节。
创建后移除 `INITIAL_PASSWORD`。公开注册不提供。

## 账号维护

Server 和 Workers 共用 `crates/nomail-app/src/auth/` 的 `AuthService`：登录、刷新、JWT、改密、初始化、设备管理及响应 DTO 都只维护一套。HTTP 层只解析请求、读取部署配置并映射错误；SQLite 事务和 D1 batch/条件更新分别实现 `AuthStore` 的原子操作。

`AuthRuntime` 提供时间、随机数和密码计算的执行方式；JWT 使用同一份 HS256/issuer/有效期校验，bcrypt 格式及 cost=10 也由 App 统一。Server 通过 `spawn_blocking` 执行密码计算，Workers 使用同一算法。刷新令牌只成功消费一次，并在同一原子操作中更新设备 IP、User-Agent 和最近活动时间；首次初始化只在整张账号表为空时发生，不覆盖软删除或已存在账号。

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

## 消息业务

Server 与 Workers 共用 App 的 `MessageService`：列表、未读数、详情、raw、已读/置顶、软删除、批量操作和接收初始化均由同一业务实现驱动。两端只保留 HTTP、存储编码与 SSE 连接适配，数据库结构和现有 API 契约不变。

查询在 App 解析为类型化 `MessageQuery`。状态更新仅写指定列，避免并发已读/置顶相互覆盖；单条操作原子检查用户及配置可见性，批量先验证全部输入，再一次更新，保持 1～100 条限制。D1 使用一个 JSON ID 数组参数，避免绑定参数超限。

接收时直接创建完整 `Message`，由运行环境传入 ID/时间，统一设置 unread、非置顶及 3 天保留期；仅首次入库返回从同一消息投影的 SSE 创建事件。平台结构、原文、等级与接收信息不会被状态更新改写。过期时间仍只作元数据，不增加自动隐藏/清理、搜索扩展、通知推送或规则引擎。

## Webhook 配置业务

配置列表、创建、修改、启停与软删除统一由 App `WebhookConfigService` 实现；两端的 `WebhookConfigStore` 只负责 SQLite/D1 编码和带用户作用域的原子操作。Key 的 16 字节随机值/十六进制格式、默认启用、凭据加密及 PATCH 规则均由 App 决定，运行环境仅提供时间、随机数和加密密钥。

PATCH 继续忽略空字符串，Secret 留空保留原凭据；全空 PATCH 不写入或更新时间。非空修改仅写对应字段，不会用旧快照覆盖并发变更，更新和返回详情保持一致。软删除、启停也在写入时检查所有权及未删除状态。配置响应不暴露明文或加密凭据，数据库结构、接收协议和公开接口不变。

## 错误处理

App 通过 `nomail_app::error` 统一导出领域错误与分类约定，错误定义仍归各业务模块；协议错误在 App 分类，不使 Core 依赖产品错误策略。HTTP 映射集中在 `crates/nomail-app/src/http/error.rs`，Server/Workers 仅适配响应和日志。D1 存储错误不再经过 HTTP 状态码再回到业务层。现有状态码、错误响应格式与公开文案保持不变。内部失败在最终响应边界记录安全诊断，不自动打印可能含凭据或请求数据的底层错误文本。

## Docker

镜像使用 Alpine Linux；构建阶段同样使用 Alpine 的 musl 工具链，运行阶段保留 CA 证书、时区数据和非 root 用户。健康检查使用内置 `wget`，无需安装 curl。

```bash
docker build -t nomail-api:rust .
docker run --rm --env-file .env -p 3000:3000 nomail-api:rust
```

`compose.yml` 挂载 `nomail-data` 到 `/data`，覆盖 `DATABASE_PATH=/data/nomail.db`。根文件系统仍只读，数据库及 WAL/SHM 在数据卷内。单独使用 `docker run` 时需添加 `-v nomail-data:/data`。
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
crates/nomail-core/src/       平台验签、解密、事件结构及基础内容提取
crates/nomail-app/src/        开源业务、存储接口、接收用例及唯一 HTTP 契约
crates/nomail-server/src/     Actix、SeaORM SQLite、设备会话、本地 SSE
crates/nomail-server/tests/   临时 SQLite 文件上的 API、持久化和并发测试
crates/nomail-workers/src/    官方 worker-rs、D1 SQL、Durable Objects SSE
crates/nomail-workers/tests/  Wrangler 本地 D1/DO 集成测试
```

例如阅读消息列表：App `http/routes.rs` 的 ListMessages → 对应运行时 `app.rs` 的 Operation 绑定 → App 消息用例及运行时存储查询；接口路径和 DTO 不在运行时重新定义。
简单接口不强制增加 Service 层。

## 测试与构建

```bash
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --all-targets
cargo test --locked -p nomail-app --features openapi --lib
cargo test --locked --workspace --doc
cargo check --locked -p nomail-core --target wasm32-unknown-unknown
cargo check --locked -p nomail-app --target wasm32-unknown-unknown
cargo check --locked -p nomail-workers --target wasm32-unknown-unknown
cargo build --locked --release -p nomail-server --bins
```

CI 统一放在 `.gitea/workflows/ci.yml`，按以下依赖关系运行：

- `test`：格式检查、Clippy、SQLite API/持久化测试和发布构建，不需要外部数据库。
- `integration`：使用官方 worker-build 和 Wrangler，在本地 D1/DO 上执行 Workers API 测试，无需 Cloudflare 账号。
- `docker`：通过 `needs: [test, integration]` 等待上述两个 Job 均成功后才构建、推送镜像。仅 `main` 分支推送、`v*` 标签推送或手动触发时执行；PR 和其他分支推送只运行检查和测试。

`test` 和 `integration` 可以并行运行；任意一项失败都会阻止镜像发布。原独立的 `docker.yml` 已合并移除，避免镜像发布绕过测试。

Server 集成测试默认运行，使用独立临时 SQLite 文件；Workers 测试使用临时本地 D1 存储，不连接远程数据库。Runner 需要 Docker 来构建发布镜像，以及 Node.js 来运行 Wrangler。

## 升级与能力边界

备份 SQLite 时应停止服务后备份数据目录，或使用 SQLite 在线备份工具；WAL 模式下不能在服务运行时只复制主数据库文件。保留 `JWT_SECRET`、`JWT_ISSUER`、`SECRET_ENCRYPTION_KEY`。Server 与 Workers 分别初始化 Schema，不支持直接互换数据库文件。

API 包含 24 项操作，覆盖 GitHub / 飞书 / Generic Webhook、幂等入库、消息管理、设备会话与 SSE。
**当前不包含通知推送或通知中转功能。** 不提供推送接口、推送状态字段或 Token 存储，不接入 APNs/FCM 或厂商通知；SSE 仅用于在线客户端实时更新收件箱。

Server SSE 为进程内广播；Workers SSE 通过 Durable Objects 按用户隔离。两者都是尽力而为、无重放，慢客户端可能丢事件。持久化任务及自动过期清理尚未实现。Workers 的 bcrypt 计算和常驻 SSE 需要评估实际套餐 CPU 与 DO 费用，不承诺免费套餐适用。

`docs/swagger.json` 可查阅接口定义；旧 PRD 和缺失接口分析是历史规划，不应替代当前路由及测试。
