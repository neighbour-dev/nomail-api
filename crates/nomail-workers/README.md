# nomail-workers

Cloudflare 官方 [workers-rs](https://github.com/cloudflare/workers-rs) 的 `worker` SDK，Rust → WASM。使用 D1 存储账号、设备、会话、Webhook 配置和消息，Durable Objects 按用户广播 SSE。不依赖 Actix、SeaORM、PostgreSQL 或 Node.js 业务框架。

## 安装与本地运行

需要 Rust 1.99+、WASM target、Node.js 22+：

```bash
rustup target add wasm32-unknown-unknown
cargo install worker-build --locked --version 0.8.7
```

在此目录执行：

```bash
npm ci
cp .dev.vars.example .dev.vars
npx wrangler d1 migrations apply DB --local
npm run dev
```

修改 `.dev.vars` 的密钥与密码。加密密钥是 Base64 的 32 个随机字节，可在仓库根目录运行 `cargo run -p nomail-server --bin gen-secret-key` 生成。`.dev.vars` 不得提交。

首次登录会在账号表为空时创建初始账号；后续不会因修改初始密码配置而重置账号。首次登录后通过 `/api/auth/account` 改密，再移除 `INITIAL_PASSWORD`。所有受保护接口除了验证 JWT，还查询设备/会话撤销状态。

## 部署

1. `npx wrangler login`。
2. `npx wrangler d1 create nomail`，将生成的数据库 ID 写入 `wrangler.toml`；全零 ID 仅用于本地测试。
3. 分别用 `npx wrangler secret put NAME` 设置：
   - `JWT_SECRET`
   - `SECRET_ENCRYPTION_KEY`
   - `INITIAL_USERNAME`
   - `INITIAL_PASSWORD`
4. `npx wrangler d1 migrations apply DB --remote` 初始化数据库。
5. `npm run deploy`，Wrangler 会调用官方 `worker-build --release`。首次部署同时创建 `Realtime` Durable Object class。

不要对远程数据库运行集成测试。密钥在升级时保持不变，否则已有 Token 或加密配置无法继续使用。

## 测试

```bash
cargo check --locked -p nomail-workers --target wasm32-unknown-unknown
npm test
```

`npm test` 使用独立临时 D1 存储，启动本地 Wrangler/DO 并检查 API、SSE、签名、软删除、无 ID 消息、并发去重、refresh token 单次消费和密码修改撤销会话。只使用测试凭据，不需要 Cloudflare 账号；本地占用 `127.0.0.1:8791`。需安装 `worker-build`，它必须在 PATH 中。

路由由 `nomail-app::http` 契约解析，Workers 只绑定 Operation，不另写路径/方法/鉴权白名单。请求、响应及公共包装来自 App；测试用生成的 OpenAPI 3.1 校验实际响应，并枚举受保护接口检查鉴权。修改契约后在仓库根目录运行 `cargo run -p nomail-app --features openapi --example export-openapi -- docs/swagger.json`；AJV 仅为测试开发依赖。

## 与 Server 的边界

- 通过 App 使用 Core 的平台协议与基础内容映射；开源版收件箱规则、Message、AuthService、DTO、分页、凭据存储加密及 Token hash 全部属于 App。依赖方向是 `Workers → App → Core`。
- 单张消息表同时保存通用字段、类型/版本/完整去重键、带标签的 platform_data JSON、可读原始内容及 NoMail 状态。列表必含 level；详情返回 metadata/platformData，原文继续使用 raw 接口。配置修改和已读/置顶操作不会重写平台快照。
- 初始 Schema 与 Server 约束对齐：完整作用域去重、平台 JSON/provider 一致性、合法等级、读状态及外键。不转换旧的本地数据库，也不会自动清空已有开发数据。
- Server 使用 SeaORM/SQLite 的原生 UUID BLOB 与日期编码；D1 使用 UUID TEXT 与规范化 UTC RFC3339 毫秒字符串。通过各自适配层保持同一 HTTP 数据契约，不直接交换数据库文件。
- `src/auth/` 只包含 HTTP/运行时组装和 AuthStore 的 D1 实现，不维护第二套 JWT、密码策略或业务流程。多步骤登录/改密使用 batch；刷新使用条件 UPDATE 与受影响行数保护，并在同一批次更新设备 IP/User-Agent/最近活动时间；不能将 Server 的持有事务对象方式搬到 Workers。
- `src/message/handler.rs` 只适配 HTTP，业务由共享 MessageService 执行。D1 MessageStore 接收类型化查询与局部状态命令，更新/详情读取使用同一 batch；批量通过 `json_each(?)` 绑定一个 JSON ID 数组，支持 100 个 ID 的单次原子写入，不用陈旧完整记录覆盖并发操作。
- `src/webhook/configs/repository.rs` 原子更新配置并返回不含 URL/凭据的 ConfigView，App HTTP 层构造公开响应；空 PATCH 不写入。
- `src/webhook.rs` 调用 App IngestionService 完成查配置、解密、验签及入箱，并通过共享 HTTP 转换获取 ACK 和 SSE 效果，Workers 只写响应和尽力而为地广播。D1 错误先转为存储/业务错误，不依赖 HTTP 状态码。
- SSE 为每用户一个 DO，单实例最多 32 个订阅、每订阅 32 条缓冲，20 秒心跳，到访问令牌过期时关闭。无重放、无离线投递，慢客户端可能丢事件；重新连接后应刷新列表。
- Core 支持的平台多于 HTTP 配置，目前只开放 Generic/GitHub/飞书。飞书仍使用一个 secret 同时作为 Token/Encrypt Key，不能自动降级认证。
- 本地测试不代表免费套餐 CPU 可满足 bcrypt cost=10。发布前用实际套餐测试登录开销；SSE 会产生 DO 使用费用。没有请求级应用限流，生产需要在 Cloudflare 配置入口限流与防滥用措施。
- 不包含通知推送或通知中转功能，没有推送接口、状态字段或 Token 存储；SSE 只负责在线收件箱更新，不接入 APNs/FCM/厂商通知。
- 没有持久化推送任务、队列、自动过期清理，也没有旧 PostgreSQL 数据迁移。

`wrangler.toml` 中的 D1 migrations 是新安装的建表初始化；不是旧数据库兼容迁移。
