# NoMail API

单用户、自托管的通知收件箱 API：接收 Webhook，将不同来源的通知保存为可管理、可追溯的消息。

## 功能

- 接收 Generic、GitHub 和飞书 Webhook，支持验签、平台验证请求与幂等入库。
- 保留平台结构化数据和可读原文，提供消息搜索、已读、置顶、软删除和批量操作。
- 提供账号、设备与会话管理，以及用于在线更新收件箱的 SSE。
- 两种部署使用同一套业务与 HTTP 接口：
  - **Server**：Rust、Actix Web、SeaORM、SQLite，可通过 Docker 或源码运行。
  - **Cloudflare Workers**：官方 worker-rs、D1、Durable Objects。

当前不包含 Web/App 客户端、公开注册或 APNs/FCM 等通知推送。SSE 无重放、无离线投递；消息过期时间暂不触发自动清理。

## 文档

- [安装与部署](docs/install.md)
- [更新日志](CHANGELOG.md)
- [Git Tag 规范](docs/tagging.md)
- [贡献指南](CONTRIBUTING.md)
- [HTTP 接口定义（OpenAPI 3.1）](docs/swagger.json)
- [平台协议库](crates/nomail-core/README.md)
- [开源版应用架构](crates/nomail-app/README.md)

## 许可证

除另有明确声明的文件及第三方组件外，本仓库的代码和文档采用 **Functional Source License 1.1，未来许可为 Apache-2.0（`FSL-1.1-ALv2`）**，许可方为 **NEIGHBOUR PTE. LTD.**，详见 [LICENSE](LICENSE)。

FSL 是源码可用许可，并非 OSI 认可的开源许可。它允许内部使用等许可用途，但限制条款所定义的竞争性商业用途。每个版本自许可方提供之日起满两年后，可按 Apache License 2.0 使用；并非整个项目在统一日期转换。

此前已按 AGPL-3.0-only 发布的版本，其既有授权不受本次变更影响。第三方组件保留其各自的许可。
