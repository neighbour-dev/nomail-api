# 更新日志

本文件记录项目面向用户的重要变更，版本遵循语义化版本规范。
项目版本统一定义在根目录 `Cargo.toml` 的 `[workspace.package].version`，四个 crate 继承同一个版本，不独立发布版本。
以 Git 标签划分已发布版本；标签之后的变更记录在 `Unreleased`，不因源码版本号尚未调整而归入旧版本。`0.1.0` 及此前开发历史仅作简要概述。

## [Unreleased]

### 新增

- 原始事件增加 `raw.header`：统一使用大小写不敏感的请求头黑名单，排除已知鉴权、Cookie 和签名头，其余运行时可见的未知/自定义头、重复值与空值全部保留。不因无关头包含非 ASCII 而拒收；Server 非 UTF-8 头值带 Base64 标记无损保存，Workers 保存运行时可见的字符串。类型化头只投影 ASCII，接收与读取规则一致；鉴权仍独立使用原有文本查询，不以 raw 的编码值替代。黑名单外的自定义敏感头仍会保存，payload 不进行递归脱敏。

- 扩展 Telegram Bot API 结构化数据：`TelegramWebhook.payload` 直接按官方 Update 请求体建模，提供 29 类更新及消息、媒体元数据、富文本实体、回调、成员状态、投票、支付和 Business 等嵌套类型。区分可访问/不可访问消息，原样保留文本与字符串标识；未知字段保留在 raw。不下载媒体、不调用 Bot API，也不接入 Telegram Gateway。

- 新增 App Store Connect 平台（`appstoreconnect`）：使用独立 Webhook Secret 验证 `x-apple-signature: hmacsha256=...`，支持官方文档列出的 12 类通知，结构化保留状态变化、地区可用性和关联资源。兼容普通与后台资源两种 relationship 结构、可缺失的 version/timestamp，按通知事件 ID 去重；不隐式调用 Apple API 补全反馈或资源详情，不混用内购/订阅的 JWS 通知协议。

- 新增独立 Gitea 平台：配置使用 `provider: gitea` 和 Webhook Secret，支持 POST JSON/表单及原生 HMAC-SHA256 验签；结构化保留 Issue/PR、审核、标签变更、Push/提交签名、引用、Fork、Wiki、Release/附件、Package、Workflow/步骤等信息。区分归一化事件和原始事件类型，保留负数系统用户 ID；手动 Replay 的新 Delivery ID 按新投递处理。Server/Workers 共用接收逻辑，Gitea 接入本身无需额外表结构迁移；本版整体升级仍须执行下述 raw 存储迁移。

- 扩展 GitHub 消息的结构化平台数据：补齐 Issue/PR 状态、指派、里程碑、审核与代码评论位置，新增 Push/引用/提交状态、Workflow Job/步骤、Check Run/Suite、Release/附件、Discussion/答案、Deployment/状态和编辑前信息，供客户端按事件展示。保持字段级宽容解析及完整 raw；仓库时间兼容字符串和 Unix 秒。平台数据响应结构的不兼容调整见下文。

### 变更

- OpenAPI JSON 不再随 Git 提交，开发和测试前按需生成本地 docs/swagger.json。CI 每次生成并校验后复用同一份产物，失败会阻止发布构建、镜像推送及 GitHub Release；发布版单独附带 swagger.json 并纳入 SHA256SUMS，不放入 Docker 或二进制压缩包。文档 info.version 自动使用项目版本，替代固定的 1.0.0。

- **平台建模不再作为接收门槛：**鉴权并完成必要解码/解密后，未知事件/版本、缺少展示元数据或无法生成平台模型不再阻止消息入库；模型不可用时使用通用展示回退，完整数据仍保存在 raw。缺少可靠事件 ID 时不猜测去重键。协议控制请求仍按原协议处理，鉴权、解密、格式解码、大小限制和存储失败不返回成功 ACK。

- **raw 接口与存储内容不兼容变更：**`GET /api/messages/{id}/raw` 使用统一 `{ code, msg, data }` JSON 响应，data 为 `{ header, payload, contentType }`：header 仅从存储 JSON 解码为条目数组，payload 始终是数据库中的原字符串，contentType 原样对应数据库 content_type；不解析 payload、不套 format/value、不按 contentType 设置 HTTP 响应头，空值、未知类型及无法解析的 payload 仍可查询。数据库分列保存过滤后的 `raw_headers`、完整解析/解密后的 `raw_payload` 及描述 payload 格式的 `content_type`；raw_payload 不含 header/format/value 包装，也不保存密文。JSON、文本、已解码表单分别按 JSON、原字符串、规范化 URL 编码保存。Core 内存中的 `PlatformEvent.raw` 仍使用 `{ header, payload: { format, value } }`。

- **平台数据改为读取投影：**删除数据库 `platform_data` 列，详情接口的 `platformData` 从消息自身的 provider 和 raw 即时生成，不重新验签、解密或读取当前配置凭据；飞书 Encrypt Key 轮换不影响已接收消息。模型失败时返回 `platformData: null` 和可排查的 `platformDataError`，正常时后者为 null，不影响详情、raw 查询、已读/置顶和删除。通用展示字段、元数据和用户状态仍以保存的记录为准。

- **平台数据模型不兼容变更：**全部平台统一为 `platformData.event = { header, payload }`。`header` 仅保存平台投递信息白名单，不包含鉴权凭据与签名；`payload` 保持已建模字段的官方路径。GitHub/Gitea 移除人为的 `push/git_ref/commit_status/wiki` 分组，PR 分支名称改从 `head.ref/base.ref` 读取；Gitea 精确事件类型移至 header。GitLab、Stripe、Slack、飞书和 Shopify 移除提升/派生字段，保留原生层级与值类型。已支持的常规事件沿用既有基础消息映射、鉴权及去重规则；模型不可用时的展示回退及缺少可靠 ID 时的接收行为见上文。不提供历史消息转换或旧格式兼容。

- **Telegram 平台数据不兼容变更：**移除自定义的 `chat_id/message_id/callback_query_id` 摘要字段，聊天、消息和回调 ID 改从 `platformData.event.payload` 下的平台原有嵌套路径读取；`update_id` 是官方请求体字段，在 payload 顶层保留。没有额外的 `update` 包装，不迁移或回填旧平台数据。

### 修复

- 平台 payload 的可缺失已建模字段区分缺失、显式 null 和实际值：仅省略不存在的 key，保留 null、空数组/对象、空字符串、false 和 0；不再补出缺失的 null/空集合。使用标准双层 Option 与统一 Serde 适配，Server/Workers 存储及返回时保留相同状态；未知或类型异常内容仍由完整 raw 兜底。

- Docker 构建阶段升级至 Alpine 3.24，运行镜像升级至包含安全修复的 Alpine 3.24.4；两个阶段均在构建时更新系统包，获取该分支已发布的安全更新。

### 升级说明

- **破坏性变更：旧消息不兼容，不提供历史消息数据迁移或兼容读取。** 升级前按需备份并暂停旧实例写入，由部署者手动物理清空消息表 `notify_messages`（包含软删除记录），无需清空整个数据库。第 3 个迁移仅新增 `raw_headers`、删除 `platform_data`；非空消息表会使迁移失败，应用不会自动删除或转换消息。Server 启动时事务性迁移，Workers 在切换新代码前应用 `0003_raw_storage.sql`。账号、设备和配置保留不变；需要保留旧消息时不能直接采用此升级方式。

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
