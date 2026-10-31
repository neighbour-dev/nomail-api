# nomail-core

不依赖 Actix、Workers SDK、数据库驱动、Tokio（仅测试使用）、环境变量或系统时钟的 Rust 业务内核。包含 Webhook 协议接收与映射、收件箱规范化、完整的社区认证业务、分页及凭据加密格式；持久化、时间、随机数和密码计算的调度由运行时提供，部署入口分别在相邻的 `nomail-server`、`nomail-workers` crate。

## HTTP 契约

`src/api/routes.rs` 的单一清单关联路径、方法、鉴权、原始/JSON 请求体、JSON/SSE/raw/平台控制响应，以及 Endpoint 关联的请求/查询/响应类型。`api::resolve()` 统一尾斜杠、静态路径优先和单次参数解码；`parse_json::<Endpoint>()` / `parse_query::<Endpoint>()` 统一输入解析，`ApiResponse<T>` / `ApiFailure` 统一响应包装与错误语义。数据库驱动和 HTTP 框架仍留在适配层。

DTO 按业务放在 auth/message/webhook 模块；消息详情和配置列表等响应也是具体 Rust 类型，不通过两端各自的 json! 拼字段。`api::EndpointResponse<E>` 表示 JSON 业务成功分支，不能用于 SSE、raw 或平台控制响应。列表 DTO 直接定义 summary/source 等线上的字段，不把存储表示当作接口类型。

启用可选 `openapi` feature 后，使用真实 DTO 的 JsonSchema 和同一路由清单生成 OpenAPI 3.1。生成器不包含另一份手写路径或响应模型；默认/WASM 生产构建不链接 Schema 生成器。导出命令在仓库根目录运行：

```bash
cargo run --locked -p nomail-core --features openapi --example export-openapi -- docs/swagger.json
cargo test --locked -p nomail-core --features openapi --lib
```

文档快照、路由唯一性、鉴权清单、DTO/查询解析和非 JSON 协议均有契约测试；两端操作绑定采用穷尽匹配，普通成功响应通过 `success::<Endpoint>()` 检查关联的响应类型。

`api/sse.rs` 统一事件名、MessageCreated/Ready 等数据 DTO、帧编码及心跳。OpenAPI 的 `x-sse-events` 是这些逻辑事件的类型 Schema；实际 HTTP 响应仍是 text/event-stream，不是 JSON 包装。平台端只管理连接、计时与广播，私有 DO 容量错误会在公开 HTTP 边界转换为共同错误格式。

## 认证业务

`src/auth/service.rs` 是唯一的登录、刷新、退出、鉴权、账号更新/初始化/本地重置和设备管理流程。`dto.rs` 定义公共请求/响应，`model.rs` 定义账号/设备/会话及鉴权上下文；不接收 ORM 实体、D1 JSON 或 HTTP 对象。

- `AuthStore`：账号/设备/会话读取，以及 `complete_login`、`rotate_refresh_token`、`update_account` 等原子操作。SQLite 与 D1 分别实现，不能把并发安全的凭据重验、Token 单次消费、设备上限和会话撤销拆成无保护的 CRUD。
- `AuthRuntime`：提供可信时间、随机字节、密码计算执行方式；bcrypt cost/格式/错误语义在 `password.rs` 中唯一实现。
- `token.rs`：只使用 jsonwebtoken 校验签名、HS256、issuer 和必要字段；到期判断显式使用传入的时间，`exp <= now` 拒绝，不依赖内部系统时钟。
- 存储命令只携带密码/Token 的哈希，不传入明文密码或明文 Refresh Token。包含凭据的配置、账号快照和令牌响应不实现 Debug；账号快照不实现 Serialize，响应使用独立 DTO。

AuthStore 接口允许非 Send future，以兼容 Workers；不强迫 D1 模拟持有事务对象。Core 测试使用固定时间和内存测试替身，原子性还由 SQLite 与 Wrangler 本地 D1 集成测试验证。

## 消息业务

`src/message/` 包含完整 Message 模型、HTTP DTO、类型化查询、`MessageService` 和 `MessageStore`，保持 `nomail_core::message::*` 公开路径与 API 契约不变。

- `MessageQuery` 统一解析分页、等级、布尔值、ID 和 UTC 时间；HTTP 规范化委托同一解析器，存储不再解析原始查询字符串。
- `MessageService` 统一消息管理与接收初始化；ID/时间显式传入，不依赖 AuthRuntime 或 HTTP 对象。
- `StatePatch` / `ReadChange` 表达局部状态变更，缺省字段不写入；`MessageMutation` 不允许修改内容、平台快照、原文或接收信息。
- `MessageStore::update_owned` 原子执行所有权/可见性检查、局部更新与更新后详情读取，不能保存陈旧完整记录覆盖并发修改。批量先验证全部输入、去重，再执行一次带作用域的写入，支持 100 个不同 ID。
- `accept` 直接构造唯一完整 Message，设置默认状态与统一 3 天保留期，存储仅负责编码。只有首次创建返回 SSE 效果，重复记录（包括已软删除记录）不再产生事件。
- 列表不加载 platform_data/raw；raw 独立读取，不依赖平台 JSON 解码。配置软删除可见性、原有 LIKE 搜索与过期元数据行为不变，不增加自动清理或新业务功能。

Core 用固定时间及内存替身测试规则；SQLite 与本地 D1 集成测试另行验证并发局部更新、用户隔离及批量边界。

## Webhook 配置管理

`webhook::config::WebhookConfigService` 统一配置 CRUD、启停、空字符串忽略/空 PATCH 不写入语义、Key 生成、默认状态和凭据加密。`WebhookConfigRuntime` 提供时间、随机字节及加密密钥；AES-GCM 格式复用 `security`。写给存储的 `NewConfig` / `ConfigPatch` 仅包含加密凭据，不实现 Debug/Serialize。

`WebhookConfigStore` 接收类型化的创建和局部变更命令，在原子操作内检查所有权/未删除状态并返回配置投影；不能先读旧快照再无条件保存所有字段。SQLite 通过事务实现更新及结果读取，D1 通过 scoped UPDATE RETURNING 实现。列表的计数和分页使用一致读取，offset 验证范围并由 D1 精确绑定。Core 单元测试验证规则，SQLite/D1 集成测试验证并发、凭据保留及跨用户隔离。接收编排和数据库结构保持不变。

## 错误约定

`nomail_core::error` 是错误的统一查找入口，导出 AuthError、MessageError、ConfigError、WebhookError 及相应 Result 别名；原有业务模块公开路径保持不变。错误定义仍位于各自的 `error.rs`，不合并为跨领域大枚举。

所有领域错误实现 `DomainError`，提供 `ErrorKind`、公开文案及安全 `Diagnostic`。Core 不打印日志，不引用 HTTP 状态码；`api/error.rs` 集中将分类映射为 400/401/404/409/500，内部错误始终返回 `internal server error`。413/429 等传输错误仍属于 ApiFailure，不扩充业务错误分类。现有响应文案与 JSON 格式不变。

Server/Workers 只在最终错误响应边界记录内部失败，中间存储→领域→HTTP 转换不记录，避免同一请求重复输出。默认诊断仅含静态领域标签和失败描述，不自动输出 Internal(String)、Display、密码、Token、数据库值或原始请求体。内部原始信息不等于安全日志；需要详细诊断时应在来源处显式提供经过审查的安全上下文，不能直接转发底层错误文本。

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
    ├── mod.rs             # PlatformEvent 信封、PlatformData 枚举、映射分发
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

`PlatformEvent` 是解析结果，`PlatformData` 枚举承载平台专有模型。平台模型的 `to_message_content(metadata, raw)` 执行内置映射，外部使用 `event.to_message_content()` 得到 `MessageContent`，它只表示通用业务字段，不是完整消息、也不是另一条消息。`nomail_core::message::Message` 同时包含通用内容、平台数据/raw 和 NoMail 管理字段。`nomail_core::webhook::GithubEvent` 等公开路径仍可用，也可按 `nomail_core::webhook::platforms::github::GithubEvent` 定位。

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
      │     三部分与通用字段一起保存为同一条 NoMail Message
      ├── Respond：验证 / Ping
      └── Ignored：无需生成业务事件
```

`raw` 的“原始”指未经消息映射的平台业务内容，不是 HTTP Body 字节：GitHub 表单取解码后的 `payload` JSON；飞书取解密后的事件 JSON；Slack 交互取内层 JSON，Slash Command 保留已解码表单字段；普通文本保留 UTF-8 文本（包含原来的空白）。`EventContent` 显式区分 `Json`、`Text`、`Form`，未知字段留在 raw，不随平台模型裁剪。

`raw.to_readable_string()` 对 JSON 格式化显示，对表单逐行展示已解码字段；`media_type()` 描述该可读表示。解析会改变 JSON 空白、字段顺序、表单编码等，**raw 不能用于重新验签或精确 HTTP 回放**。签名仍只使用 `WebhookRequest.body`。Core 不保留整套 HTTP 头或密文字节，不实现传输报文归档。

平台模型只提取已支持的关键语义：例如 GitHub 的仓库和 action、Stripe 的资源引用、飞书的消息类型、Slack 的交互 action、Telegram 的聊天和消息标识。它不是另一份完整 JSON 副本。完整结构在 raw；映射可同时使用语义模型与 raw，不隐式调用外部 API。

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

`to_message_content()` 是纯映射：生成标题、正文、来源、必填 MessageLevel 和链接，不生成 ID、不读取系统时间、不设置用户状态、不调用外部 API。`apply_inbox_rules()` 原地处理默认值与长度，替代原来的重复中间模型；存储时仍保留平台结构和 raw。NoMail 的接收时间、已读、置顶、过期/删除时间由应用流程管理，不写进平台解析结果。

`MessageLevel` 为 `low / normal / high / urgent`，定义 `priority_rank()` / `is_at_least()`。这是关注优先级，不是结果或故障严重程度。`from_label()` 支持这四个标签（去首尾空白、不区分大小写），未知或旧标签回退 normal；严格解析/列表筛选拒绝其他值。平台成功、失败等状态保留在 data/raw，没有明确优先级规则时默认 normal。优先级不决定通知或删除：`DEFAULT_RETENTION_DAYS` 当前统一为 3 天，不实现通知发送或规则引擎。

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

WASM 检查确保 core 可编译，不代表已经完成 Workers HTTP/D1/广播/队列适配。
