# 更新日志

本文件记录项目面向用户的重要变更，版本遵循语义化版本规范。
项目版本统一定义在根目录 `Cargo.toml` 的 `[workspace.package].version`，四个 crate 继承同一个版本，不独立发布版本。
以 Git 标签划分已发布版本；标签之后的变更记录在 `Unreleased`，不因源码版本号尚未调整而归入旧版本。`0.1.0` 及此前开发历史仅作简要概述。

## [Unreleased]

## [0.4.0] - 2026-11-12

### 新增

- 原始事件增加请求头记录，排除已知鉴权、Cookie 和签名头，保留其余运行时可见头及重复值；支持非 ASCII 值。**自定义敏感头可能仍被保存，payload 不自动脱敏。**

- 扩展 Telegram Bot API 数据模型，支持 29 类更新及消息、媒体元数据、回调、支付和 Business 等嵌套数据；不下载媒体或调用 Bot API。字段路径的不兼容调整见下文。

- 新增 App Store Connect 平台（`appstoreconnect`），使用独立 Webhook Secret，支持 12 类通知并按事件 ID 去重；不调用 Apple API 补全详情，也不处理内购/订阅 JWS 通知。

- 新增独立 Gitea 平台（`gitea`），支持 Secret 验签、POST JSON/表单及 Issue/PR、Push、Release、Package、Workflow 等事件。手动 Replay 的新 Delivery ID 按新投递处理。本版升级仍须执行下述消息存储迁移。

- 扩展 GitHub 数据模型，补齐 Issue/PR、审核、Push、Workflow、Check、Release、Discussion、Deployment 及编辑前信息。响应结构的不兼容调整见下文。

### 变更

- OpenAPI JSON 不再随 Git 提交，开发和测试前需生成；发布版单独附带 `swagger.json` 及校验和，不放入 Docker 或二进制压缩包。`info.version` 改用项目版本，替代固定的 `1.0.0`。

- **接收行为变化：**鉴权及必要解码/解密成功后，未知事件/版本、缺少展示元数据或平台建模失败不再阻止入库，使用通用展示回退并保留 raw。缺少可靠事件 ID 时不去重。控制请求处理不变，鉴权、解密、格式解码、大小限制和存储失败仍不会返回成功 ACK。

- **raw 接口不兼容变更：**`GET /api/messages/{id}/raw` 不再直接返回原文，改用 `{ code, msg, data }` JSON 包装，data 为 `{ header, payload, contentType }`。header 是头条目数组，payload 始终为存储的字符串（包括 JSON），contentType 是数据字段，不用于设置 HTTP 响应头。空值、未知类型及无法解析的 payload 仍可查询；客户端需调整原文解析和下载逻辑。

- **平台数据改为读取投影：**`platformData` 从已保存的 provider 和 raw 即时生成，不依赖当前凭据，后续模型更新可能改变投影结果。详情及状态更新响应中，该字段可为 null，并新增 `platformDataError` 说明原因，正常时后者为 null；客户端不能再假设模型始终可用。通用展示字段、元数据和用户状态仍以保存值为准，投影失败不影响消息管理和 raw 查询。

- **平台数据模型不兼容变更：**全部平台统一为 `platformData.event = { header, payload }`，分别承载白名单投递头和原生层级的已建模字段。移除人为分组、提升字段及派生字段；GitHub PR 分支改从 `event.payload.pull_request.head.ref` 和 `event.payload.pull_request.base.ref` 读取，GitLab、Stripe、Slack、飞书和 Shopify 需按原生路径及值类型适配。具体类型见 [Core 文档](crates/nomail-core/README.md)。不提供旧格式兼容。

- **Telegram 字段迁移：**移除 `chat_id/message_id/callback_query_id` 摘要字段，改从 `platformData.event.payload` 下相应消息或回调分支读取。`update_id` 位于 payload 顶层，没有额外 `update` 包装，不回填旧平台数据。

### 修复

- 平台 payload 区分字段缺失、显式 null 和实际值：不再为缺失字段补 null 或空集合，保留实际提供的空值、false 和 0。依赖固定字段集合的客户端需适配。

- Docker 构建环境升级至 Alpine 3.24，运行镜像升级至 Alpine 3.24.4，并在构建时安装该分支的系统安全更新。

### 升级说明

**旧消息不兼容，不提供历史消息转换或兼容读取。需要保留旧消息时，不可直接采用以下升级方式。**

1. 备份数据库与配置，停止旧实例和请求写入。
2. 由部署者手动物理清空 `notify_messages`，包括软删除记录；仅在客户端删除消息不够。无需清空整个数据库，账号、设备和配置可保留。
3. 应用第 3 个迁移：新增 `raw_headers`、删除 `platform_data`。Server 启动时事务性执行；Workers 在切换新代码前应用 `0003_raw_storage.sql`。**消息表非空会导致迁移失败，应用不会自动删除消息。**
4. 同步更新客户端对 raw 响应、平台字段路径和可空投影的处理。迁移后的数据库不能直接交回旧程序使用；回退需恢复升级前的备份。

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
