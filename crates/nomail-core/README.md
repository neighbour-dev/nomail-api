# nomail-core

独立的平台协议库，负责 Webhook 平台配置类型与校验、验签、解密、解析、控制请求识别、平台事件及通用内容提取。不依赖账号系统、数据库、HTTP 框架、环境变量、系统时钟或异步运行时，可供开源版及未来 SaaS 分别使用。

开源版账号、接收配置记录的管理、收件箱规则、接收编排、凭据存储加密和 HTTP 契约均在相邻的 [nomail-app](../nomail-app/README.md)。平台专属配置不属于收件箱业务，由 Core 定义；Core 不引用 App，不定义 NoMail API 路由、ACK 包装或 SSE。

可选 `openapi` feature 只为协议数据派生 JSON Schema，供应用生成自己的 OpenAPI；Core 不生成产品 API 文档。

## 代码组织

按平台纵向组织，而不是将平台实现分散到全局 auth/parse/mapping 文件：

```text
src/webhook/
├── mod.rs                 # 公共 receive 入口与平台分发
├── config.rs              # 平台判别枚举、统一入口与安全摘要，不堆放平台实现
├── types.rs               # 共享请求、配置、结果、消息、错误
├── crypto.rs              # 共用密码学工具与时间窗口校验
├── support.rs             # 共用 JSON/表单、字段与数据构造工具
├── tests.rs               # 跨平台公共契约测试
├── test_support.rs        # 共用测试工具，不放平台样例
└── platforms/
    ├── mod.rs             # PlatformEvent 信封、PlatformData 枚举、映射分发
    ├── generic/
    ├── github/
     │   ├── mod.rs         # GitHub 事件类型、验签、解析、内置映射
     │   ├── config.rs      # GitHub 凭据类型、纯校验及认证参数构造
    │   └── tests.rs       # GitHub 样例、签名构造与测试
    ├── gitlab/
    ├── stripe/
    ├── slack/
    ├── feishu/
    ├── telegram/
    └── shopify/
```

每个平台目录均包含 `mod.rs`、`config.rs` 和 `tests.rs`，修改一个平台只需进入对应目录。配置类型、凭据校验与认证参数构造由平台自己负责；公共 `PlatformConfig` 仅按 provider 分发，不集中实现各平台认证。没有 Parser trait，也不要求各平台采用相同的内部流程。

`PlatformEvent` 是解析结果，`PlatformData` 枚举承载平台专有模型。平台模型的 `to_message_content(metadata, raw)` 执行基础映射，外部使用 `event.to_message_content()` 得到 `MessageContent`，它只表示平台内容，不是带 NoMail 管理状态的完整消息。持久化和产品规则由调用方决定。`nomail_core::webhook::GithubEvent` 等公开路径仍可用，也可按 `nomail_core::webhook::platforms::github::GithubEvent` 定位。

## 事件模型与可读原始内容

```text
WebhookRequest（签名用的原始字节）+ 可信配置
  → 验证、解密、解码、平台解析
  → ReceiveOutcome
      ├── Event(Box<PlatformEvent>)
      │     ├── metadata：事件类型、版本、带作用域的去重标识
      │     ├── data：PlatformData，各平台有意义的专有字段
      │     └── raw：EventContent，人类可读的平台完整原始内容
      │           → to_message_content() → 通用业务字段 MessageContent
      │     由调用方决定如何持久化及应用产品规则
      ├── Respond：验证 / Ping
      └── Ignored：无需生成业务事件
```

`raw` 的“原始”指未经消息映射的平台业务内容，不是 HTTP Body 字节：GitHub 表单取解码后的 `payload` JSON；飞书取解密后的事件 JSON；Slack 交互取内层 JSON，Slash Command 保留已解码表单字段；普通文本保留 UTF-8 文本（包含原来的空白）。`EventContent` 显式区分 `Json`、`Text`、`Form`，未知字段留在 raw，不随平台模型裁剪。

`raw.to_readable_string()` 对 JSON 格式化显示，对表单逐行展示已解码字段；`media_type()` 描述该可读表示。解析会改变 JSON 空白、字段顺序、表单编码等，**raw 不能用于重新验签或精确 HTTP 回放**。签名仍只使用 `WebhookRequest.body`。Core 不保留整套 HTTP 头或密文字节，不实现传输报文归档。

平台模型提取用于展示和交互的业务语义：例如 GitHub 的事件资源、状态、审核、代码位置及执行步骤，Stripe 的资源引用、飞书的消息类型、Slack 的交互 action、Telegram 的聊天和消息标识。它不是另一份完整 JSON 副本。完整结构在 raw；映射可同时使用语义模型与 raw，不隐式调用外部 API。

GitHub 的公开类型位于 `webhook::platforms::github`，模型定义集中在 `github/models.rs`。`GithubEvent` 保留原有字段，并提供 `review/thread`、`workflow/workflow_job`、`check_run/check_suite`、`release`、`discussion/answer/old_answer`、`deployment/deployment_status`、`label/assignee/milestone/changes`。根级业务字段分别归入 `push`、`git_ref`（create/delete）和 `commit_status`（status）。Issue 的 `pull_request` 引用用于区分 PR 会话评论；PR 保留 `head_branch/base_branch` 并补充 `head/base`。`GithubTimestamp` 保留仓库时间原来的字符串或 Unix 秒类型。

接收仍先验签并解码为 JSON，再逐字段提取；单个字段类型异常不会使整个事件被拒绝或丢弃其他有效字段。正文、代码片段和编辑前正文保留空白，未知字段仍在 raw。新增字段使用默认值以兼容旧的持久化快照；不会从旧 raw 隐式重建历史平台数据，也不改变现有通用摘要或优先级规则。

Generic 明确是 **NoMail 自定义协议**，不是任意平台自动识别器。`GenericEvent::Json` 表达 JSON 字段，`GenericEvent::Text` 表达 `X-NoMail-*` 请求头输入；没有 `GenericDefaults`。JSON 分支不继承文本分支的请求头。非 UTF-8 文本现在显式拒绝，不用有损替换伪装成可读原文。

## 使用

```rust
use nomail_core::webhook::{
    receive, Authentication, Provider, ReceiveOutcome, WebhookConfig, WebhookRequest,
};

// 示例是显式允许匿名访问的 Generic 端点。
// 有密钥的端点应使用 HmacSha256 等认证方式，不能在验签失败时回退为 None。
let config = WebhookConfig::new(Provider::Generic, Authentication::None, 1_793_354_400);
let headers = vec![("Content-Type".into(), "application/json".into())];
let body = br#"{"id":"evt-1","title":"Backup","content":"Done","level":"normal"}"#;

match receive(&config, &WebhookRequest { headers: &headers, body })? {
    ReceiveOutcome::Event(event) => {
        let message = event.to_message_content();
        assert_eq!(message.title, "Backup");
        let key = &event.metadata().deduplication_key;
        // 适配层把通用字段、event.metadata、event.data、event.raw
        // 与 NoMail 管理状态原子写入同一行，成功后再返回 ACK。
        assert_eq!(event.raw.as_json().unwrap()["id"], "evt-1");
        // event.data 用于平台展示/交互，event.raw 保留可读完整内容。
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

`receive()` 是安全接入入口，将配置对应的认证、必要的解密/解码和解析放在同一流程中，避免调用方忘记验签。输出业务请求时是 `ReceiveOutcome::Event(Box<PlatformEvent>)`（Box 避免放大控制结果的枚举大小）；控制请求不伪装成消息。

`to_message_content()` 是纯映射：提取标题、正文、来源、必填 MessageLevel 和链接，不生成 ID、不读取系统时间、不设置用户状态、不调用外部 API。默认文案、字段长度、接收时间、已读、置顶、过期/删除规则均由上层产品决定，不写进平台解析结果。

`MessageLevel` 为 `low / normal / high / urgent`，定义 `priority_rank()` / `is_at_least()`。这是关注优先级，不是结果或故障严重程度。`from_label()` 支持这四个标签（去首尾空白、不区分大小写），未知或旧标签回退 normal；严格解析拒绝其他值。平台成功、失败等状态保留在 data/raw，没有明确优先级规则时默认 normal。Core 不决定保留期、通知或删除行为。

## 协议范围

| 平台 | 接收能力 | 内置映射 |
|---|---|---|
| Generic | HMAC-SHA256 或显式无认证；JSON/UTF-8 文本 | JSON 字段与文本请求头分别建模，再映射消息 |
| GitHub | 原始 JSON/表单 Body HMAC；事件头；Ping | Issues、PR、评论、Push、Workflow Run 摘要 |
| GitLab | 固定 Token 或 Standard Webhooks 签名；轮换签名；时间窗口 | MR/评论摘要；自定义模板保留并使用 JSON 回退 |
| Stripe | 原始 Body + 时间戳签名；多个 v1 签名；Snapshot/Thin | 事件摘要；Thin 明确标记未获取详情 |
| Slack | 原始 Body + 时间戳签名；Events API、Interaction、Slash Command、URL 验证 | 文本/命令摘要；交互保留 JSON；限流通知忽略 |
| 飞书 | Verification Token；拼接 SHA256 签名；AES-256-CBC；URL 验证；v2/旧版信封 | 消息文本二次 JSON 解码与通用摘要 |
| Telegram | 固定 Secret Token；Update 类型分流 | 文本、caption、按钮 data，非文本使用 JSON 回退 |
| Shopify | Base64 HMAC；主题、API 版本、店铺、投递与事件 ID | title/name 摘要，裁剪字段使用 JSON 回退 |

事件结构采用平台专属类型和必要的子类型（例如 `StripeEvent::Snapshot/Thin`、`SlackEvent::EventsApi/Interaction/SlashCommand`），完整 JSON 保留在 `raw: EventContent::Json`。**当前只建模已支持的关键字段，不是所有上游事件的完整强类型 Schema。** 未知字段不会丢失；未知普通事件一般使用 raw 摘要。版本保留在元数据中，并不表示已验证所有 API 版本的字段兼容性。

GitLab 自定义模板、Shopify 字段裁剪不要求完整资源。Stripe Thin 不会隐式补充查询；如果以后需要详情，应在适配层提供显式的异步补全步骤。交互请求只聚合为通知，不实现 Slack/Telegram 的完整交互机器人流程。

## 配置与安全边界

- `PlatformConfig` 用 `provider` + `credentials` 的带标签枚举自动反序列化，平台凭据位于 `platforms/<provider>/config.rs`。严格拒绝未知字段、跨平台凭据和位置数组；`validate()` 执行纯凭据校验，`receiving_config(now)` 同时推导 Provider 和 Authentication，避免调用方独立组合错误的平台与认证。
- 秘密配置不实现 Debug；Serialize 仅用于调用方的加密存储，不能用于公开响应。`summary()` 返回无秘密的 `PlatformSummary`（provider、authentication）。密文存储格式、数据库和密钥管理仍由调用方负责。
- 新强类型配置只允许 Generic 显式 `credentials.type=none`；其他平台必须提供对应凭据。低层 `WebhookConfig` / `Authentication` 继续可供协议测试和独立调用方使用，但不会使产品配置接口自动接受匿名平台配置。
- Provider 和认证方式必须来自可信端点配置，不能由请求头决定；认证失败不降级。
- `Authentication::None` 是明确允许不认证的策略，不是“自动探测”。生产环境有 Secret 时应显式配置对应认证方式。
- 同一端点只有一种预期认证策略。GitLab 新版签名缺失不回退旧 Token。
- `WebhookConfig::now` 由调用方传入 Unix 秒；时间戳签名默认允许前后 300 秒，处理 GitLab、Stripe、Slack 和普通飞书签名事件。可调整 `timestamp_tolerance`，调用方必须使用可信时钟。
- 飞书将 Verification Token 和 Encrypt Key 分开配置。普通事件配置 Encrypt Key 时必须有签名，并在解密前验证原始外层 Body；URL 验证走单独 Token 流程，可接收加密包装。没有普通签名的加密请求仅为识别和认证 URL 验证而解密，不可据此接受普通事件。
- `HeaderLookup` 实现必须不区分请求头大小写，并保留重复头。提供的 `Vec<(String, String)>` 实现满足要求；轮换签名不会因覆盖同名项丢失。
- 接收层应限制 Body 大小、请求频率、处理时长；raw 可能包含个人信息、平台 Token、临时响应地址等敏感字段，必须受访问控制和保留策略保护，不应无条件写日志或把整个事件公开给 App。可读并不意味着可公开。
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

WASM 检查确保协议库可编译，不依赖 Workers HTTP/D1/广播等部署适配。
