# Rust 迁移说明

> 历史文档：记录 Go → Rust/PostgreSQL 阶段，不再描述当前部署。当前公开版为 `nomail-server` + SQLite、`nomail-workers` + D1；没有旧数据迁移需求，也不保留 PostgreSQL 兼容。以根 README 和 Workers README 为准。

## 范围

Actix Web + SeaORM 替换原服务端，保留现有 27 个路由、单用户策略和 PostgreSQL 表结构。
账号重置和生成密钥也已提供 Rust 二进制。仓库不再依赖 Go 工具链。

## 兼容约定

- 响应仍为 `code/msg/data`，成功 HTTP 200，错误码使用对应 HTTP 状态码。
- 路由接受尾部斜杠，JSON 字段沿用 camelCase。
- 保留 bigint 账号/配置/设备 ID、UUID v7 消息 ID、软删除、历史可空字符串字段。
- JWT 保持 HS256、`user_id/session_id/sub/iss/iat/exp`；鉴权还会查询会话是否有效。
- Refresh Token 使用随机十六进制字符串，数据库只存 SHA-256，轮换使用行锁事务。
- bcrypt 密码兼容；新密码继续使用 cost=10，超过 72 字节明确拒绝。
- 敏感信息保持 AES-256-GCM，标准 Base64 编码 `nonce[12] || ciphertext || tag[16]`，不使用 AAD。
- Webhook 使用收到的原始字节验签；GitHub/Generic 使用 HMAC-SHA256，飞书保留签名及 verification token 两种验证方式。
- `(webhook_config_id, external_id)` 作为幂等键，软删除后仍返回原消息 ID。
- 消息等级、3/7/30 天过期时间计算和批量操作维持现有规则。
- SSE 保留 ready、message.created、auth.expired、20 秒心跳和 32 条订阅缓冲。
- 配置 PATCH 的空字符串仍忽略，不借迁移改变客户端语义。
- 健康检查保留每个来源 IP 两秒一次的限流。

## 数据库升级

1. 停止旧服务写入，备份数据库。
2. 在备份恢复的隔离数据库上启动 Rust 服务，检查表结构及数据读写。
3. 原有六张表会被基线迁移接管，不重建表，不重新生成 ID，不重新加密数据。
4. 新增 `seaql_migrations` 表记录版本；基线不会修复与原模型不一致的自定义 Schema。
5. 生产保留同一组密钥及连接配置，切换镜像后验证登录、刷新、Webhook、消息及 SSE。

禁止执行基线 down，避免删除接管的用户数据。迁移采用版本控制，不再在每次启动隐式同步 ORM 模型。
若原数据库使用了非标准字段类型或缺少现有字段，需要先进行专门的 Schema 迁移。

应用层继续写入原表，因此回退旧镜像通常不需要回退数据；生产前必须使用实际备份验证。
不要随机分流旧/新实例并期待 SSE 跨进程互通，也不要同时启用两套 Schema 迁移器。

## 有意保持的边界

不新增 APNs/FCM 发送、队列、过期清理、用户注册或 SQLite。历史 PRD 中这些规划不属于本次等价迁移。
默认不信任 X-Forwarded-For，来源 IP 取 TCP 对端；反向代理后需要另行配置可信代理策略。

## 验证

`cargo test` 执行纯逻辑测试；PostgreSQL 集成测试单独执行，覆盖全部路由、跨账号隔离、并发刷新、软删除去重、设备限制、SSE 及账号密码重置。
测试连接不要使用生产数据库；测试会创建/删除自己的随机 Schema。

`tests/fixtures/legacy-crypto.json` 是迁移前实现实际生成的固定样本（测试密钥，不含真实凭据）；`legacy-schema.sql` 根据旧模型实际建表结果整理。测试验证旧 JWT、bcrypt 哈希、AES-GCM 密文及可空历史字段能继续使用。
