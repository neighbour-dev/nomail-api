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
├── raw.rs                 # 原始头、统一黑名单、完整解码内容封装
├── crypto.rs              # 共用密码学工具与时间窗口校验
├── support.rs             # 共用 JSON/表单、字段与数据构造工具
├── tests.rs               # 跨平台公共契约测试
├── test_support.rs        # 共用测试工具，不放平台样例
└── platforms/
    ├── mod.rs             # PlatformEvent 信封、PlatformData 枚举、映射分发
    ├── generic/
    ├── app_store_connect/ # App Store Connect HMAC、通知信封、资源关系及官方示例测试
    ├── github/
    │   ├── mod.rs         # GitHub 验签、解析、内置映射
    │   ├── config.rs      # GitHub 凭据类型、纯校验及认证参数构造
    │   ├── models.rs      # GitHub 事件与资源模型
    │   └── tests.rs       # GitHub 样例、签名构造与测试
    ├── gitlab/
    ├── gitea/             # 原生验签、独立资源模型、Gitea 事件映射与测试
    ├── stripe/
    ├── slack/
    ├── feishu/
    ├── telegram/
    └── shopify/
```

每个平台目录均包含 `mod.rs`、`config.rs` 和 `tests.rs`，修改一个平台只需进入对应目录。配置类型、凭据校验与认证参数构造由平台自己负责；公共 `PlatformConfig` 仅按 provider 分发，不集中实现各平台认证。没有 Parser trait，也不要求各平台采用相同的内部流程。

`PlatformEvent` 是解析结果，`PlatformData` 枚举承载平台专有模型。每个分支统一为 `XxxWebhook { header: XxxWebhookHeader, payload: XxxWebhookPayload }`；例如 `nomail_core::webhook::GithubWebhook`，细分类型位于 `nomail_core::webhook::platforms::github`。平台模型的 `to_message_content(metadata, raw)` 执行基础映射，外部使用 `event.to_message_content()` 得到 `MessageContent`，它只表示平台内容，不是带 NoMail 管理状态的完整消息。持久化和产品规则由调用方决定。

## 事件模型与可读原始内容

```text
WebhookRequest（签名用的原始字节）+ 可信配置
  → 验证、解密、解码、平台解析
  → ReceiveOutcome
      ├── Event(Box<PlatformEvent>)
      │     ├── metadata：事件类型、版本、带作用域的去重标识
      │     ├── provider：可信接收配置的平台，不依赖模型是否成功
      │     ├── data：Option<PlatformData>，尽力生成的 XxxWebhook
      │     │     ├── header：平台投递信息（白名单字段，不含鉴权凭据/签名）
      │     │     └── payload：平台事件参数，保留原有字段与嵌套层级
      │     └── raw：RawWebhook
      │           ├── header：黑名单过滤后的原始头条目列表
      │           └── payload：EventContent，完整解码内容（Json / Text / Form）
      │           → to_message_content() → 通用业务字段 MessageContent
      │     由调用方决定如何持久化及应用产品规则
      ├── Respond：验证 / Ping
      └── Ignored：无需生成业务事件
```

`raw.payload` 的“原始”指未经消息映射的平台业务内容，不是 HTTP Body 字节：GitHub 表单取解码后的 `payload` JSON；飞书取解密后的事件 JSON；Slack 交互取内层 JSON，Slash Command 保留已解码表单字段；普通文本保留 UTF-8 文本（包含原来的空白）。`EventContent` 使用 `{ "format": "json|text|form", "value": ... }` 区分内容类型，未知字段留在 raw.payload，不随平台模型裁剪。通用消息映射只接收这部分内容，不会把原始头拼进通知正文。

类型化 header 仅提取白名单中的 ASCII 值；非 ASCII 值留在 raw，不额外导致鉴权后的拒收。正文和 JSON 支持 UTF-8。接收和 raw 重建共用模型解析入口。

`raw.header` 是 `{ "name": "...", "value": "..." }` 条目列表，保留重复项和空值，不合并或裁剪。过滤按通用 `RAW_HEADER_BLACKLIST` 和各平台声明的黑名单精确名称、大小写不敏感匹配，省略已知鉴权、Cookie 和签名头；未知头保留。`RawHeader::from_bytes()` 将非 UTF-8 值编码为 Base64 并标记 `encoding: "base64"`；编码值不能替代原始鉴权输入。捕获范围取决于调用方的运行时，无法恢复已被规范化的线上报文。

黑名单由 Core 统一聚合：通用 HTTP 项维护在 `webhook/raw.rs`，平台专属项维护在各自的 `webhook/platforms/<平台>/mod.rs`，所有平台和两端共用完整过滤范围；只有成功鉴权并产出业务事件后才将过滤结果附加到 raw。**黑名单之外的自定义敏感头也会保存**，新增认证头时必须同步维护对应黑名单，新增平台时还需将其列表加入聚合。此策略只过滤 header，不扫描或删改 payload 中的同名业务字段。

`raw.to_readable_string()` / Display 序列化 Core 内存中的 `{ header, payload: { format, value } }`，`raw.media_type()` 描述该内存封装的 JSON 序列化格式；它不是 raw HTTP 查询接口的响应 DTO。原始 HTTP Content-Type 若存在则在 raw.header 中，内存中的解码类型由 payload.format 标识。

`RawWebhook::stored_payload()` 和 `payload_content_type()` 提供仅含解码 payload 的序列化文本及格式；`from_stored()` 从头条目、payload 文本和格式恢复 raw。`parse_raw(provider, raw)` 仅生成平台模型，不重新验签、解密、检查投递时间窗口或读取凭据。产品存储及查询投影由 [App](../nomail-app/README.md) 定义。

**raw 不能用于重新验签或精确 HTTP 回放**：验签使用接收流程中的原始字节，JSON 空白、字段顺序、表单编码、密文和线级 HTTP 报文不归档。未知事件/版本、缺失展示元数据及模型不支持的合法 JSON 仍可产出带 raw 的事件，模型不可用时采用通用展示回退。鉴权、必要解密及格式解码失败仍返回错误；已识别的控制请求按平台协议处理。

`data.header` 只保存白名单内的平台事件类型、投递 ID、版本等信息，与采用黑名单的 raw.header 不同。`data.payload` 按平台原有路径提取已支持字段，不混入 NoMail 摘要或 HTTP 头；来源可以是 JSON、解码后的表单或文本。没有附加投递信息时，header 为 `{}`；Generic 纯文本的 payload 是字符串。

payload 的可缺失已建模字段采用 `JsonField<T> = Option<Option<T>>`，保留三种状态：

| Rust 值 | JSON 含义 |
|---|---|
| `None` | key 缺失，序列化时省略 |
| `Some(None)` | 显式 null |
| `Some(Some(value))` | 实际值，包含空集合、空字符串、false 和 0 |

Serde 适配保留三态；`JsonFieldExt` 仅方便展示映射访问非空值，不改变序列化。类型异常字段留在 raw，模型必填字段缺失时 data 可以为 `None`，不影响已鉴权解码事件的产出。header 的可选字段使用普通 `Option<String>`。

raw.payload 完整保留解码内容，包括上游放在业务内容中的字段；HTTP 头的黑名单过滤不代表对 payload 做了脱敏，例如飞书业务 JSON 内的验证 Token 仍属于完整解码内容。通用标题、正文、来源等由 `MessageContent` 映射生成，不混进平台 payload；不隐式调用外部 API。

GitHub 模型位于 `github/models.rs`，保持上游字段路径；PR 分支使用 `head.ref/base.ref`。`GithubTimestamp` 支持字符串和 Unix 秒，label 保留字符串或对象类型。完整字段以模型定义为准。

接收先验签并解码，再逐字段提取；单个可选字段类型异常不丢弃其他有效字段。字符串原样提取，不将数字/布尔值转成字符串，不裁剪文本。产品版本的不兼容变更及升级要求见 [CHANGELOG.md](../../CHANGELOG.md)。

Generic 明确是 **NoMail 自定义协议**，不是任意平台自动识别器。`GenericWebhook.header` 表达 `X-NoMail-*` 输入；`payload` 是支持的 JSON 字段、原样文本或 JSON null。JSON 分支的通用展示映射不继承文本分支的请求头。非 UTF-8 文本显式拒绝，不用有损替换伪装成可读原文。

## 使用

### Gitea

`PlatformConfig` 的 Gitea 分支使用 Secret，验证 Gitea 原生签名，不使用 GitHub 或 Gogs 兼容头替代。

Gitea 使用独立模型（依据上游 v28.3.0），不复用 GitHub 类型。`metadata.event_type` 来自 `X-Gitea-Event`；header 另外保留精确事件类型与 Delivery ID。payload 保持原生层级，用户 ID 为有符号整数，工作流 ID 为字符串。

验签始终针对 Gitea 的原始 JSON 文本：JSON 投递使用原 Body，表单投递先提取解码后的 `payload`，不能重新序列化再验签，也不能对整个编码表单验签。去重使用 `gitea:delivery` 与原生 Delivery ID；Gitea 手动 Replay 生成新 UUID，因此会作为新消息处理。Gitea 的测试投递是普通 Push，不是 GitHub Ping。

### App Store Connect

`PlatformConfig` 的 `appstoreconnect` 分支使用独立 Webhook Secret，不是 API 私钥/JWT 或内购共享密钥。以原始 JSON Body 验证 `x-apple-signature`，前缀为 `hmacsha256=`。

`AppStoreConnectWebhook.payload.data` 保持官方 `type/id/version/attributes/relationships` 层级，header 为空对象。relationship 同时支持普通与后台资源结构，Rust 调用方可通过 `reference()` / `self_url()` 统一访问；version、timestamp 及 instance 并非全局必填。

事件类型、版本和去重标识分别来自 `data.type`、`data.version`、`data.id`，scope 为 `appstoreconnect:event`，不按资源 ID 合并。ID 为字符串，不要求 UUID；反馈通知仅表示资源创建，不包含完整反馈，Core 不调用 Apple API 补全。

未知通知按普通事件处理，不根据管理 API 的 `WebhookPing/webhookPings` 类型推测入站控制请求。该平台不是 App Store Server Notifications，不处理 `signedPayload`/JWS。

测试将官方 `webhook-events` 的 14 个示例（12 类事件）固化为脱敏 JSON fixture，并逐字段检查类型和值；CI 不依赖外部样本、Apple 账号或联网下载。

### Stripe

Snapshot 与 Thin 使用独立信封类型，序列化保持原生 JSON 层级，不增加分支包装；这不表示响应字段集合与旧模型完全相同。Snapshot 的 `created` 是 Unix 秒整数，Thin 是时间字符串；Thin 模型包含 `reason.request`、`related_object` 和 `snapshot_event` 等可选字段，只投影实际收到的内容，不调用 Stripe API 补全详情。

资源模型依据官方公开 GA OpenAPI `2026-10-28.endive`（固定 commit），生成 200 个 live/deleted 资源分支及其嵌套类型。`data.object` 根据原生 `object` 分派，可展开字段区分 ID、展开资源及已删除资源。整数金额、十进制字符串、普通数值字段与原生列表/字典分别保留类型；状态枚举仍以字符串保留未来值。`previous_attributes` 保持稀疏旧值映射，未知资源使用原生对象回退；未知顶层信封无法生成平台模型，但仍保留 raw 并采用通用展示回退。已鉴权解码的投递不因建模失败而丢失。

这些资源分支不是事件类型清单，也不意味着一条消息会包含所有资源或字段。实际投递取决于事件订阅、业务活动和所用 Stripe 产品；一些模型用于描述关联资源或 API 展开结构，Webhook 不一定直接携带。Thin 通常只有轻量通知，可展开字段也可能只有 ID；UI 应按实际收到的类型和字段适配，不能把缺失字段当作零值或已补全的详情。

这不是所有 Stripe 历史版本的严格 Schema：已知资源的未建模/类型异常字段及历史版本特有字段保留在 raw，也不表示实现了 Issuing 同步授权决策。来源、生成检查命令、字段语义和兼容边界见 [Stripe 模型说明](src/webhook/platforms/stripe/README.md)。测试固化 16 个官方脱敏资源示例，覆盖 200 个资源分支的判别与往返序列化，以及典型嵌套字段、三态、可展开对象和验签后的 raw 重建；不代表穷举验证了所有嵌套字段或上游事件组合。

### 通用接收示例

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
        // 调用方原子持久化所需数据，成功后再返回 ACK。
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

`to_message_content()` 是纯映射：提取标题、正文、来源、必填 MessageLevel 和链接，不生成 ID、不读取系统时间、不设置用户状态、不调用外部 API。Core 提供平台基础文案及模型不可用时的通用展示回退，不改写平台模型或 raw；上层产品再执行字段规范化、长度限制，并决定接收时间、已读、置顶、过期和删除规则。

`MessageLevel` 为 `low / normal / high / urgent`，定义 `priority_rank()` / `is_at_least()`。这是关注优先级，不是结果或故障严重程度。`from_label()` 支持这四个标签（去首尾空白、不区分大小写），未知或旧标签回退 normal；严格解析拒绝其他值。平台成功、失败等状态保留在 data/raw，没有明确优先级规则时默认 normal。Core 不决定保留期、通知或删除行为。

## 协议范围

| 平台 | 接收能力 | 内置映射 |
|---|---|---|
| Generic | HMAC-SHA256 或显式无认证；JSON/UTF-8 文本 | JSON 字段与文本请求头分别建模，再映射消息 |
| GitHub | 原始 JSON/表单 Body HMAC；事件头；Ping | Issues、PR、评论、Push、Workflow Run 摘要 |
| Gitea | JSON 原文或表单解码后 JSON HMAC；原生事件/精确类型头；POST | Issue/PR/审核、Push、引用、Fork、Wiki、Release、Package、Commit Status、Workflow/Job 摘要与独立资源模型 |
| App Store Connect | POST JSON 原文 HMAC-SHA256；`x-apple-signature` | App/Build/Beta/后台资源状态、反馈资源创建、替代分发可用性和资源关系；不隐式查询 API |
| GitLab | 固定 Token 或 Standard Webhooks 签名；轮换签名；时间窗口 | MR/评论摘要；自定义模板保留并使用 JSON 回退 |
| Stripe | 原始 Body + 时间戳签名；多个 v1 签名；独立 Snapshot/Thin 信封及 200 个生成资源分支 | 现有事件标题与原文正文；Thin 明确标记未获取详情，不联网补全；详细模型不等于已实现专属展示卡片 |
| Slack | 原始 Body + 时间戳签名；Events API、Interaction、Slash Command、URL 验证 | 文本/命令摘要；交互保留 JSON；限流通知忽略 |
| 飞书 | Verification Token；拼接 SHA256 签名；AES-256-CBC；URL 验证；v2/旧版信封 | 消息文本二次 JSON 解码与通用摘要 |
| Telegram | 固定 Secret Token；Update 类型分流 | 文本、caption、按钮 data，非文本使用 JSON 回退 |
| Shopify | Base64 HMAC；主题、API 版本、店铺、投递与事件 ID | title/name 摘要，裁剪字段使用 JSON 回退 |

事件结构采用平台专属的 `XxxWebhook { header, payload }`。Stripe 根据 payload.object 区分 Snapshot/Thin，保留 `data.object` 和 `related_object.type` 原有位置；Slack 保留 Events API 的 `event` 层、交互的 `team/user` 对象及 Slash Command 原有参数；飞书保留 payload 内原生 `header/event/sender_id`，不再生成 `content_json`。**各平台的模型覆盖范围不同：Stripe 按固定版本官方资源规范生成详细类型，其他平台的具体字段以各自模型定义为准；这些都是读取投影，不是覆盖所有上游事件及 API 版本的严格校验 Schema。** 未建模内容由 raw 完整保留，部分未知对象还提供原生对象回退；未知普通事件一般使用 raw 摘要。版本保留在元数据中，并不表示已验证所有 API 版本的字段兼容性。

Telegram 模型依据 [Bot API Update](https://core.telegram.org/bots/api#update)（2026-11-10），不包括 Telegram Gateway：

- payload 顶层为必填 `update_id` 和 29 类可选更新分支，无额外 `update` 包装；header 为空，不含 Secret Token。
- 嵌套字段宽容提取；联合类型按 `type/status/source` 或消息 `date` 分流，未知变体保留为未知对象。
- 文本、回调数据及字符串 ID 原样保存；实体偏移使用 UTF-16 单元。媒体只有标识与元数据，不下载内容。
- 接收允许仅有 `update_id`、缺少模型字段或多个更新分支的已鉴权信封；模型可能不可用，但 raw 保留。没有有效 update_id 时不猜测去重键。

GitLab 自定义模板、Shopify 字段裁剪不要求完整资源。Stripe Thin 不会隐式补充查询；如果以后需要详情，应在适配层提供显式的异步补全步骤。交互请求只聚合为通知，不实现 Slack/Telegram 的完整交互机器人流程。

## 配置与安全边界

- `PlatformConfig` 用 `provider` + `credentials` 的带标签枚举自动反序列化，平台凭据位于 `platforms/<provider>/config.rs`。严格拒绝未知字段、跨平台凭据和位置数组；`validate()` 执行纯凭据校验，`receiving_config(now)` 同时推导 Provider 和 Authentication，避免调用方独立组合错误的平台与认证。
- 秘密配置不实现 Debug；Serialize 仅用于调用方的加密存储，不能用于公开响应。`summary()` 返回无秘密的 `PlatformSummary`（provider、authentication）。密文存储格式、数据库和密钥管理仍由调用方负责。
- `PlatformConfig` 只允许 Generic 显式 `credentials.type=none`；其他平台必须提供对应凭据。低层 `WebhookConfig` / `Authentication` 可供协议测试和独立调用方使用，不替代高层配置校验。
- Provider 和认证方式必须来自可信端点配置，不能由请求头决定；认证失败不降级。
- `Authentication::None` 是明确允许不认证的策略，不是“自动探测”。生产环境有 Secret 时应显式配置对应认证方式。
- 同一端点只有一种预期认证策略。GitLab 新版签名缺失不回退旧 Token。
- `WebhookConfig::now` 由调用方传入 Unix 秒；时间戳签名默认允许前后 300 秒，处理 GitLab、Stripe、Slack 和普通飞书签名事件。可调整 `timestamp_tolerance`，调用方必须使用可信时钟。
- 飞书将 Verification Token 和 Encrypt Key 分开配置。普通事件配置 Encrypt Key 时必须有签名，并在解密前验证原始外层 Body；URL 验证走单独 Token 流程，可接收加密包装。没有普通签名的加密请求仅为识别和认证 URL 验证而解密，不可据此接受普通事件。
- `HeaderLookup` 鉴权查询必须不区分头名称大小写并保留重复值；`entries()` 必须枚举全部运行时可见头，含重复和空值。非文本值不能有损转换为可匹配的凭据。
- 接收层应限制 Body 大小、请求频率、处理时长；raw 可能包含个人信息、平台 Token、临时响应地址等敏感字段，必须受访问控制和保留策略保护，不应无条件写日志或把整个事件公开给 App。可读并不意味着可公开。
- `EventMetadata` 的去重键包含 scope/value，数据库必须再增加用户/租户及端点作用域。Telegram 端点必须绑定固定 Bot 身份；切换 Bot 时需更换端点或在存储作用域中加入 Bot 身份。
- 没有可靠事件/投递 ID 时去重键是 `None`，不使用资源 ID 冒充事件 ID，也不自动哈希 Body。Slack Interaction/Slash Command 暂不提供推测的去重键。
- Shopify 的操作 ID 与投递 ID 分开保留。当前去重选择投递 ID，业务是否合并同一操作由后续用例决定。
- 一次 `Event` 只代表按配置完成认证及必要的解码/解密，不保证平台模型生成成功，也不代表已可靠接收；`data` 可以为 `None`。应原子持久化消息及必要任务，再确认；异步平台截止时间需要接收层自行保证。

## 检查

```sh
cargo test -p nomail-core --all-targets
cargo test -p nomail-core --doc
cargo check -p nomail-core --target wasm32-unknown-unknown
```

WASM 检查确保协议库可编译，不依赖 Workers HTTP/D1/广播等部署适配。
