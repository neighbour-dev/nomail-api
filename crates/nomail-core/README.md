# nomail-core

不依赖 Actix、Workers SDK、数据库、Tokio、环境变量或系统时钟的 Rust 业务内核。目前实现 Webhook 协议接收与内置收件箱消息映射；尚未包含持久化、推送、队列或 Workers 部署入口。

## 代码组织

按平台纵向组织，而不是将平台实现分散到全局 auth/parse/mapping 文件：

```text
src/webhook/
├── mod.rs                 # 公共 receive 入口与平台分发
├── types.rs               # 共享请求、配置、结果、消息、错误
├── crypto.rs              # 共用密码学工具与时间窗口校验
├── support.rs             # 共用 JSON/表单、字段与数据构造工具
├── tests.rs               # 跨平台公共契约测试
├── test_support.rs        # 共用测试工具，不放平台样例
└── platforms/
    ├── mod.rs             # PlatformEvent、类型重导出、方法转发
    ├── generic/
    ├── github/
    │   ├── mod.rs         # GitHub 事件类型、验签、解析、内置映射
    │   └── tests.rs       # GitHub 样例、签名构造与测试
    ├── gitlab/
    ├── stripe/
    ├── slack/
    ├── feishu/
    ├── telegram/
    └── shopify/
```

每个平台目录均包含 `mod.rs` 和 `tests.rs`，修改一个平台只需进入对应目录。认证策略校验由平台自己负责；外层 `Authentication` 仅保留现有配置枚举，不集中实现各平台认证。没有 Parser trait，也不要求各平台采用相同的内部流程。

平台事件自身提供 `metadata()` 与 `to_message()`；`PlatformEvent` 的同名方法仅转发，不包含平台业务规则。原有 `nomail_core::webhook::GithubEvent` 等公开路径继续可用，也可按 `nomail_core::webhook::platforms::github::GithubEvent` 定位。

## 使用

```rust
use nomail_core::webhook::{
    receive, Authentication, Provider, ReceiveOutcome, WebhookConfig, WebhookRequest,
};

// 示例是显式允许匿名访问的 Generic 端点。
// 有密钥的端点应使用 HmacSha256 等认证方式，不能在验签失败时回退为 None。
let config = WebhookConfig::new(Provider::Generic, Authentication::None, 1_793_354_400);
let headers = vec![("Content-Type".into(), "application/json".into())];
let body = br#"{"id":"evt-1","title":"Backup","content":"Done","level":"success"}"#;

match receive(&config, &WebhookRequest { headers: &headers, body })? {
    ReceiveOutcome::Event(event) => {
        let message = event.to_message();
        assert_eq!(message.title, "Backup");
        let key = &event.metadata().deduplication_key;
        // 适配层用 message + key 原子入库，成功后再返回平台要求的 ACK。
        // event 保留平台 Payload，便于详情展示或后续自定义映射。
    }
    ReceiveOutcome::Respond(response) => {
        // 验证/Ping 等控制请求；转换 status、content_type、body 后直接返回。
        assert!((200..300).contains(&response.status));
    }
    ReceiveOutcome::Ignored { reason: _ } => {
        // 已通过认证但无需生成消息；按平台协议确认。
    }
}
# Ok::<(), nomail_core::webhook::WebhookError>(())
```

`receive()` 是安全接入入口，将配置对应的认证、必要的解密/解码和解析放在同一流程中，避免调用方忘记验签。输出业务请求时是 `ReceiveOutcome::Event(PlatformEvent)`；控制请求不伪装成消息。

`to_message()` 采用 Rust 命名规范，对应概念上的 `toMessage`。它是纯函数，使用内置规则生成 `IncomingMessage`，不生成数据库主键、不设置用户/租户、不执行保留策略、不调用外部 API。平台原始事件与收件箱消息是两种不同模型。

## 协议范围

| 平台 | 接收能力 | 内置映射 |
|---|---|---|
| Generic | HMAC-SHA256 或显式无认证；JSON/文本 | title/content/message/body、消息级别、文本请求头默认值 |
| GitHub | 原始 JSON/表单 Body HMAC；事件头；Ping | Issues、PR、评论、Push、Workflow Run 摘要 |
| GitLab | 固定 Token 或 Standard Webhooks 签名；轮换签名；时间窗口 | MR/评论摘要；自定义模板保留并使用 JSON 回退 |
| Stripe | 原始 Body + 时间戳签名；多个 v1 签名；Snapshot/Thin | 事件摘要；Thin 明确标记未获取详情 |
| Slack | 原始 Body + 时间戳签名；Events API、Interaction、Slash Command、URL 验证 | 文本/命令摘要；交互保留 JSON；限流通知忽略 |
| 飞书 | Verification Token；拼接 SHA256 签名；AES-256-CBC；URL 验证；v2/旧版信封 | 消息文本二次 JSON 解码与通用摘要 |
| Telegram | 固定 Secret Token；Update 类型分流 | 文本、caption、按钮 data，非文本使用 JSON 回退 |
| Shopify | Base64 HMAC；主题、API 版本、店铺、投递与事件 ID | title/name 摘要，裁剪字段使用 JSON 回退 |

事件结构采用平台专属类型和必要的子类型（例如 `StripeEvent::Snapshot/Thin`、`SlackEvent::EventsApi/Interaction/SlashCommand`），Payload 保留 `serde_json::Value`。**当前实现的是协议信封和内置映射，不是所有上游事件的完整强类型 Schema。** 未知字段不会丢失；未知普通事件一般使用 JSON 摘要。版本保留在元数据中，并不表示已验证所有 API 版本的字段兼容性。

GitLab 自定义模板、Shopify 字段裁剪不要求完整资源。Stripe Thin 不会隐式补充查询；如果以后需要详情，应在适配层提供显式的异步补全步骤。交互请求只聚合为通知，不实现 Slack/Telegram 的完整交互机器人流程。

## 配置与安全边界

- Provider 和认证方式必须来自可信端点配置，不能由请求头决定；认证失败不降级。
- `Authentication::None` 是明确允许不认证的策略，不是“自动探测”。生产环境有 Secret 时应显式配置对应认证方式。
- 同一端点只有一种预期认证策略。GitLab 新版签名缺失不回退旧 Token。
- `WebhookConfig::now` 由调用方传入 Unix 秒；时间戳签名默认允许前后 300 秒，处理 GitLab、Stripe、Slack 和普通飞书签名事件。可调整 `timestamp_tolerance`，调用方必须使用可信时钟。
- 飞书将 Verification Token 和 Encrypt Key 分开配置。普通事件配置 Encrypt Key 时必须有签名，并在解密前验证原始外层 Body；URL 验证走单独 Token 流程，可接收加密包装。没有普通签名的加密请求仅为识别和认证 URL 验证而解密，不可据此接受普通事件。
- `HeaderLookup` 实现必须不区分请求头大小写，并保留重复头。提供的 `Vec<(String, String)>` 实现满足要求；轮换签名不会因覆盖同名项丢失。
- 接收层应限制 Body 大小、请求频率、处理时长；映射输出可能包含敏感 Payload，不应无条件写日志。
- `EventMetadata` 的去重键包含 scope/value，数据库必须再增加用户/租户及端点作用域。Telegram 端点必须绑定固定 Bot 身份；切换 Bot 时需更换端点或在存储作用域中加入 Bot 身份。
- 没有可靠事件/投递 ID 时去重键是 `None`，不使用资源 ID 冒充事件 ID，也不自动哈希 Body。Slack Interaction/Slash Command 暂不提供推测的去重键。
- Shopify 的操作 ID 与投递 ID 分开保留。当前去重选择投递 ID，业务是否合并同一操作由后续用例决定。
- 一次 `Event` 只代表认证与解析成功，不代表已可靠接收。应原子持久化消息及必要任务，再确认；异步平台截止时间需要接收层自行保证。

## 检查

```sh
cargo test -p nomail-core --all-targets
cargo test -p nomail-core --doc
cargo check -p nomail-core --target wasm32-unknown-unknown
```

WASM 检查确保 core 可编译，不代表已经完成 Workers HTTP/D1/广播/队列适配。
