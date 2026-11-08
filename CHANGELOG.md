# 更新日志

本文件记录项目面向用户的重要变更，版本遵循语义化版本规范。
项目版本统一定义在根目录 `Cargo.toml` 的 `[workspace.package].version`，四个 crate 继承同一个版本，不独立发布版本。
以 Git 标签划分已发布版本；标签之后的变更记录在 `Unreleased`，不因源码版本号尚未调整而归入旧版本。`0.1.0` 及此前开发历史仅作简要概述。

## [Unreleased]

## [0.3.0] - 2026-11-07

### 新增

- 新增 `GET /api/messages/cursor`，支持 `before` 加载历史、`after` 升序分批获取新增消息，复用消息列表筛选及预览结构，返回首尾及续查游标，不统计总数。原 `page` 分页接口保持不变；Server 和 Workers 通过新增迁移创建游标查询复合索引。

### 变更

- **SSE API 不兼容变更：**`message.created` 的 `data` 仅携带消息 `id` 和服务端接收时间 `receivedAt`（UTC，与消息接口的 `createdAt` 一致），不再携带标题、来源、级别、状态等消息字段。客户端收到通知后应通过消息列表或详情接口获取数据；连接恢复后仍需重新查询收件箱，不支持事件重放。
- GitHub Release 正文改为使用更新日志中对应版本的内容；发布前校验版本条目，缺失、重复或为空时停止发布。二进制附件和 SHA-256 校验文件保持不变。

### 升级说明

- 从 `0.2.0` 升级时可保留现有数据库及配置；Server 启动时自动应用游标索引迁移，Workers 需按安装指南应用新增 D1 迁移。迁移只新增索引，不新增业务列或删除数据；升级前仍应备份。
- 依赖旧 SSE 消息字段的客户端必须同步调整；游标查询是接收顺序分页，不是可靠同步日志，详见[消息游标查询](https://git.neighbour.sg/neighbour/nomail-docs/src/branch/main/reference/message-cursor.md)。

## [0.2.0] - 2026-11-04

### 新增

- Webhook 配置开放 GitLab、Stripe、Slack、Telegram 和 Shopify，与 Generic、GitHub、飞书统一使用 Core 的强类型平台配置。
- 标签发布流程新增 Linux AMD64、ARM64 原生可执行程序压缩包及 SHA-256 校验文件，包含服务端、密钥生成和账号重置工具；补充无需 Docker 的二进制安装说明。

### 变更

- **Webhook API 不兼容变更：**创建配置必须提供 `name` 和 `platform`，按 `platform.provider` 选择对应的 `credentials` 类型；移除旧顶层 `provider`、`secret` 及配置 `source`。严格拒绝未知字段、错误类型和不完整的平台替换。消息自身的来源字段不受影响。
- PATCH 省略字段表示保留，提供 `platform` 时整体替换平台及凭据；空请求不更新时间，`description` 可用空字符串清除。查询只返回平台及认证模式摘要，不回传凭据。
- 飞书 Verification Token 与 Encrypt Key 独立配置，支持仅 Token 或加密事件模式；Generic 无认证需显式选择，其他平台必须提供对应凭据；Stripe 支持多个签名密钥用于轮换。
- 平台配置、纯校验及认证参数构造按平台目录归入 Core；App 保留配置管理、所有权、启停和整体加密持久化，Server 与 Workers 共用同一套契约。
- **数据库不兼容变更：**SQLite/D1 新建表基线以 `encrypted_platform` 替代 `encrypted_secret`，移除配置 `source`，不提供旧数据库迁移。升级前须备份，使用新数据库初始化并重建配置；旧账号、消息等不会自动导入，也不会自动清理原数据。Server 在启动时拒绝缺少新配置列的旧数据库。
- 许可证由 AGPL-3.0-only 调整为 FSL-1.1-ALv2，许可方为 NEIGHBOUR PTE. LTD.；每个版本提供满两年后可按 Apache-2.0 使用。此前已按 AGPL 发布版本的既有授权不受影响，具体条款见 [LICENSE](LICENSE)。
- 安装指南改用已发布的 GHCR 镜像，补充二进制安装、配置及升级注意事项；平台参考文档扩展为七个平台的接收协议与配置协议，清理过时的需求与迁移说明。

### 修复

- 飞书配置 Encrypt Key 后，拒绝普通事件以明文形式投递，即使签名正确也不能绕过配置的加密要求；URL 验证仍使用独立的 Token 校验流程。

## [0.1.0] - 2026-11-03

首个带版本标签的发布，汇总此前开发成果：

- 提供单用户、自托管通知收件箱后端，支持 Server（Actix Web + SQLite）和 Cloudflare Workers（D1 + Durable Objects）两种运行方式；包含账号与设备会话、Webhook 接收配置、消息管理、原文查看和在线 SSE 更新。
- 产品接入 Generic、GitHub 和飞书 Webhook，支持来源认证、平台验证请求与幂等入库；独立 Core 提供更多平台协议解析能力。
- 提供 OpenAPI、部署与贡献文档，以及原生双架构 CI、GHCR 多架构镜像和标签自动发布流程；支持代理来源 IP 解析，健康检查不再限流。

<!-- 发布新版本时，将 Unreleased 条目移入实际版本号和发布日期下，保留 Unreleased；不补改已发布标签的范围。 -->
