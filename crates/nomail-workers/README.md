# nomail-workers

Cloudflare 官方 [workers-rs](https://github.com/cloudflare/workers-rs) 的 `worker` SDK，Rust → WASM。使用 D1 存储账号、设备、会话、Webhook 配置和消息，Durable Objects 按用户广播 SSE。不依赖 Actix、SeaORM、PostgreSQL 或 Node.js 业务框架。

## 安装与部署

环境准备、本地运行、D1/Secret 配置及远程部署统一见[安装指南](https://git.neighbour.sg/neighbour/nomail-docs/src/branch/main/guide/install.md)。开发检查和提交要求见[贡献指南](../../CONTRIBUTING.md)，本文件仅补充 Workers 的实现边界。

## 测试

```bash
cargo check --locked -p nomail-workers --target wasm32-unknown-unknown
npm test
```

`npm test` 使用独立临时 D1 存储，启动本地 Wrangler/DO 并检查 API、SSE、签名、软删除、无 ID 消息、并发去重、refresh token 单次消费和密码修改撤销会话。只使用测试凭据，不需要 Cloudflare 账号；本地占用 `127.0.0.1:8791`。需安装 `worker-build`，它必须在 PATH 中。

路由和接口执行统一由 `nomail-app::http::execute` 完成，Workers 不再绑定 Operation、解析 DTO 或编写逐接口 Handler。`src/app.rs` 只实现请求/响应适配、广播与订阅，`src/services.rs` 组装 D1 存储、配置和运行时。测试还用生成的 OpenAPI 3.1 对实际 JSON 响应做 Schema 校验，并从文档枚举全部受保护接口检查鉴权。修改契约后先在仓库根目录运行 `cargo run -p nomail-app --features openapi --example export-openapi -- docs/swagger.json`；AJV 仅为测试开发依赖，不进入 Worker 运行包。

## 与 Server 的边界

- 不额外以非 ASCII 请求头拒收消息；raw 保留黑名单之外的运行时可见字符串，类型化头仅投影 ASCII，正文和 JSON 支持 UTF-8。HTTP 传输语法由运行时处理，鉴权和必要解密不降级。
- 通过 App 使用 Core 的平台协议与基础内容映射；开源版收件箱规则、Message、AuthService、DTO、分页、凭据存储加密及 Token hash 全部属于 App。依赖方向是 `Workers → App → Core`。
- 单张消息表保存通用字段、类型/版本/完整去重键、raw 和 NoMail 状态，不再有 platform_data 列。raw_headers 为黑名单过滤后的请求头 JSON；raw_payload 只保存完整解码/解密数据，不包 header/format/value；content_type 描述该数据的 JSON、文本或表单格式。详情里的 platformData 由 App/Core 从 raw 即时解析，不重新验签、不访问当前 Webhook 凭据；飞书 Key 轮换不影响历史解密数据读取。列表不加载 raw，不解析平台数据；状态修改不重写 raw。
- 最新 Schema 与 Server 约束对齐：完整作用域去重、raw_headers JSON 数组、合法等级、读状态及外键。第 3 个迁移只调整存储列，要求消息表物理清空，不转换旧记录，也不自动清空开发数据。
- Server 使用 SeaORM/SQLite 的原生 UUID BLOB 与日期编码；D1 使用 UUID TEXT 与规范化 UTC RFC3339 毫秒字符串。通过各自适配层保持同一 HTTP 数据契约，不直接交换数据库文件。
- `src/auth/` 只包含密码执行和 AuthStore 的 D1 实现，不维护第二套 JWT、密码策略或业务流程。多步骤登录/改密使用 batch；刷新使用条件 UPDATE 与受影响行数保护，并在同一批次更新设备 IP/User-Agent/最近活动时间；不能将 Server 的持有事务对象方式搬到 Workers。
- `src/message/` 只保留存储适配。D1 MessageStore 的更新/详情记录读取使用同一 batch，按所有权和可见性原子写入；平台模型失败不阻止入库、详情或状态操作，详情返回 platformData=null 和 platformDataError，不把建模失败变成已提交写入后的 500。批量通过 `json_each(?)` 绑定一个 JSON ID 数组，支持 100 个 ID 的单次原子写入，不用陈旧完整记录覆盖并发操作。
- `src/webhook/configs/repository.rs` 原子更新配置并返回不含 URL/凭据的 ConfigView，App HTTP 层构造公开响应；空 PATCH 不写入。
- 查配置、解密、验签及入箱由 App IngestionService 完成，App HTTP 执行器转换 ACK 和 SSE 效果，Workers 只写响应和尽力而为地广播。D1 错误先转为存储/业务错误，不依赖 HTTP 状态码。
- SSE 为每用户一个 DO，单实例最多 32 个订阅、每订阅 32 条缓冲，20 秒心跳，到访问令牌过期时关闭。无重放、无离线投递，慢客户端可能丢事件；重新连接后应刷新列表。
- HTTP 配置开放 Generic 及 GitHub/Gitea/App Store Connect/GitLab/Stripe/Slack/飞书/Telegram/Shopify，使用 Core 按平台目录定义的严格 platform 配置；Gitea 要求 Secret，支持 POST JSON/表单并使用原生签名头；App Store Connect 使用 `appstoreconnect` 和 Webhook Secret，按通知事件 ID 去重，不查询 Apple API；飞书的 Token/Encrypt Key 独立，Generic 匿名需显式选择。App 整体加密配置并返回无秘密的平台摘要，D1 原子更新 provider 与 encrypted_platform；配置不再有 source。不提供当前基线之前的单 Secret 配置数据库迁移。
- 本地测试不代表免费套餐 CPU 可满足 bcrypt cost=10。发布前用实际套餐测试登录开销；SSE 会产生 DO 使用费用。没有请求级应用限流，生产需要在 Cloudflare 配置入口限流与防滥用措施。
- 不包含通知推送或通知中转功能，没有推送接口、状态字段或 Token 存储；SSE 只负责在线收件箱更新，不接入 APNs/FCM/厂商通知。
- 没有持久化推送任务、队列、自动过期清理，也没有旧 PostgreSQL 数据迁移。

`wrangler.toml` 中的 D1 migrations 包含基线、游标索引及 0003_raw_storage.sql，不提供旧 PostgreSQL 或单 Secret 配置数据库迁移。升级前按需备份，暂停旧 Worker 和请求写入，由部署者物理清空 notify_messages，应用第 3 个表结构迁移后再启用新版；非空表会使迁移失败并回滚，账号、设备和配置无需清空，不提供消息 JSON 转换或旧格式兼容。

raw 查询接口使用普通 `{ code, msg, data }` JSON 响应，data 为 `{ header, payload, contentType }`，直接对应数据库的 raw_headers/raw_payload/content_type。header 只从 JSON 存储文本解码成条目数组，不重新过滤；payload 始终是原字符串，不按 MIME 解析，不增加 format/value；contentType 原样返回且不用于设置 HTTP 响应头。未知 MIME、空字符串和无法解析的 payload 仍可取回排查。表单保存为 URL 编码字符串，文本原样保存，JSON 保存为 JSON 文本。入箱时 HTTP 头只按 Core 黑名单排除；未知敏感头也会保留，body 不自动脱敏。
