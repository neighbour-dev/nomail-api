# nomail-workers

Cloudflare 官方 [workers-rs](https://github.com/cloudflare/workers-rs) 的 `worker` SDK，Rust → WASM。使用 D1 存储账号、设备、会话、Webhook 配置和消息，Durable Objects 按用户广播 SSE。不依赖 Actix、SeaORM、PostgreSQL 或 Node.js 业务框架。

## 文档范围

本文件只说明 Workers 适配层、D1 存储与 Durable Objects 实现。

- 安装、配置、费用及生产防护：[Workers 部署指南](https://git.neighbour.sg/neighbour/nomail-docs/src/branch/main/guide/install/workers.md)。
- 开发环境与完整检查：[贡献指南](../../CONTRIBUTING.md#tests-and-checks)。
- 共享业务、raw 存储与 HTTP 执行：[App 架构](../nomail-app/README.md)。
- 不兼容变更与升级前置要求：[更新日志](../../CHANGELOG.md#升级说明)。

## 测试

在仓库根目录检查 WASM 编译：

```bash
cargo check --locked -p nomail-workers --target wasm32-unknown-unknown
```

运行集成测试前，按贡献指南准备工具、安装 npm 依赖并重新生成 OpenAPI，然后在本 crate 目录执行：

```bash
npm test
```

`npm test` 使用独立临时 D1 存储，启动本地 Wrangler/DO 并检查 API、SSE、签名、软删除、无 ID 消息、并发去重、refresh token 单次消费和密码修改撤销会话。只使用测试凭据，不需要 Cloudflare 账号；本地占用 `127.0.0.1:8791`。需安装 `worker-build`，它必须在 PATH 中。

测试在模块加载时读取本地 `docs/swagger.json`，用于实际响应的 Schema 校验及受保护接口的鉴权检查，不能等到测试回调中才生成。AJV 仅为测试依赖，不进入 Worker 运行包。

## 请求与运行时适配

- 依赖方向为 `Workers → App → Core`。路由、DTO、鉴权策略和业务编排由 App 统一执行，Workers 不维护第二套接口 Handler。
- `src/app.rs` 适配请求、响应及实时广播；`src/services.rs` 组装 D1 存储、配置和运行时；`src/auth/` 实现密码执行与 AuthStore。
- HTTP 语法由 Workers 运行时处理。raw 捕获运行时可见的头字符串，不尝试恢复线上原始字节；过滤及类型化投影规则由 Core 定义。
- D1 错误先转换为存储/业务错误，不依赖 HTTP 状态码。

## D1 存储

- D1 使用 UUID TEXT 和 UTC RFC3339 毫秒时间字符串；Server 使用原生 UUID BLOB 与日期编码。适配层保持同一 HTTP 契约，但数据库文件不能直接交换。
- Schema 与 Server 的业务约束对齐，包括作用域去重、raw_headers JSON 数组、合法等级、读状态及外键；具体定义见 `migrations/`。
- 登录与改密的多步写入使用 batch。刷新令牌通过条件 UPDATE 和受影响行数保证单次消费，同批更新设备信息；不要照搬 Server 的持有事务对象方式。
- 消息状态更新和结果读取在同一 batch 内执行，原子检查所有权和可见性。批量操作通过 `json_each(?)` 绑定 JSON ID 数组，不用陈旧完整记录覆盖并发更新。
- 配置替换原子更新 provider 与 encrypted_platform，空 PATCH 不写入。

## Durable Objects 与 SSE

每用户一个 DO；单实例最多 32 个订阅，每订阅最多缓冲 32 条事件，20 秒心跳，到访问令牌过期时关闭连接。广播尽力而为，无重放或离线投递，慢客户端可能丢事件。

上述限制不构成可靠消息队列；客户端重连后需要重新查询收件箱。
