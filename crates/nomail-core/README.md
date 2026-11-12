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
     │   ├── mod.rs         # GitHub 事件类型、验签、解析、内置映射
     │   ├── config.rs      # GitHub 凭据类型、纯校验及认证参数构造
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

不对运行时已接受的请求增加整请求 ASCII 限制：无关头包含非 ASCII 不应导致消息丢失。类型化 header 仅提取白名单中的 ASCII 值，非 ASCII 留在 raw，不导致鉴权通过后的业务消息拒收。接收和 raw 重建都走同一个模型解析入口，规则一致；中文标题可通过 Generic JSON 的 title 字段提供，正文和 JSON 内的业务 header 对象仍支持 UTF-8。

`raw.header` 保存运行时可见请求头，仅排除 `src/webhook/raw.rs` 中 `RAW_HEADER_BLACKLIST` 精确列出的名字，按 ASCII 大小写不敏感匹配；不使用平台白名单或模糊词匹配。Authorization、Cookie、Secret Token、各平台签名等已知敏感头直接省略，不以占位值替代。未知头、投递 ID、时间戳、nonce、自定义头及空字符串值均保留。条目结构为 `{ "name": "...", "value": "..." }`；Server 非 UTF-8 字节值使用 Base64 并标记 encoding=base64，不以编码后的字符串参与鉴权。Workers 保存运行时可见字符串。列表保留重复项，不主动合并、拆分或裁剪；无法恢复运行时已规范化的线上原始形式。

黑名单集中在 Core，所有平台和两端共用；只有成功鉴权并产出业务事件后才将过滤结果附加到 raw。**黑名单之外的自定义敏感头也会保存**，新增认证头时必须同步维护黑名单。此策略只过滤 header，不扫描或删改 payload 中的同名业务字段。

`raw.to_readable_string()` / Display 序列化 Core 内存中的 `{ header, payload: { format, value } }`，`raw.media_type()` 描述该内存封装的 JSON 序列化格式；它不是 raw HTTP 查询接口的响应 DTO。原始 HTTP Content-Type 若存在则在 raw.header 中，内存中的解码类型由 payload.format 标识。

`GET /api/messages/{id}/raw` 走 App 的普通 JSON 响应流程，返回 `{ code: 0, msg: "success", data: { header, payload, contentType } }`。header 只把 raw_headers 的 JSON 存储文本解码为数组，不重新过滤或建模；payload 始终是 raw_payload 原字符串（JSON 也不解析成对象），contentType 原样对应 content_type。该查询不调用 Core 的 from_stored/parse_raw，不校验或归一化 payload 与 contentType，因此空字符串、未知 MIME 和不可解析的历史/异常内容都能用于排查。contentType 不用于设置接口响应头。

数据库只分别保存 `raw_headers`（过滤后的头条目 JSON）、`raw_payload`（完整解码/解密后的数据）和 `content_type`（raw_payload 的格式），不再保存 platform_data 列，也不在 raw_payload 内增加 header/format/value 包装。JSON 数据保存为 JSON 文本，文本数据保存原字符串；已解码的表单/参数按 application/x-www-form-urlencoded 规范序列化以便无损恢复字符串参数。content_type 分别为 application/json、text/plain 或 application/x-www-form-urlencoded，绝不因为 raw 查询接口返回 JSON 就全部写成 application/json。参数和 body 均按协议处理后的结果保存；当前产品接收路由仍只有 POST，没有自行增加 GET 投递支持。

读取详情时，App 使用消息自身保存的 provider 和三项 raw 字段，调用 Core 的 `parse_raw()` 生成 `platformData`；不会调用接收用例重新验签、检查过期的投递时间窗口、解密密文或读取当前 Webhook 凭据。飞书保存的是解密后的完整 JSON，后续 Encrypt Key 轮换不影响历史消息读取。新增/修正平台模型只影响读取投影，不需要迁移历史平台快照。通用标题、正文、优先级、接收时间、去重及用户状态仍按入箱时的业务记录保存，不在读取时重新生成。

**raw 不能用于重新验签或精确 HTTP 回放**：签名仍只使用原始 `WebhookRequest.body`，JSON 空白、字段顺序、表单编码、密文和线级 HTTP 报文不归档。模型解析失败不是丢弃消息的理由：未知事件/版本、缺失展示元数据及模型不支持的合法 JSON 仍保存 raw，模型不可用时采用通用展示回退。没有可靠事件 ID 时不生成猜测的去重键。详情的 platformData 可以为 null，并由 platformDataError 提供诊断，不影响已读/置顶/删除；raw 查询不依赖平台模型。鉴权、必要解密/格式解码、大小限制和存储失败仍必须报错，不以成功 ACK 掩盖失败；已识别的控制请求仍按平台协议处理。

`data.header` 保存明确建模的平台事件类型、投递 ID、版本等信息，不是任意 HTTP 头集合；未知头、Authorization、Secret Token、签名等不会复制进去，与采用黑名单的 raw.header 不同。`data` 中的 `payload` 对已支持字段按平台原有路径提取，不混入 NoMail 摘要或请求头信息；其来源可以是 JSON、解码后的表单或文本，不靠 HTTP method 划分类型。没有需要保留的附加投递信息时，`data.header` 为 `{}`，Generic 纯文本的类型化 payload 则是字符串。传输支持范围不因模型调整而改变，产品接收接口仍只支持 POST。

模型不是原始请求归档，未建模和类型异常的内容仍完整保留在 raw。payload 的可缺失已建模字段采用 `JsonField<T> = Option<Option<T>>`：`None` 表示没有这个 key，序列化时省略；`Some(None)` 表示显式 JSON null，输出 null；`Some(Some(value))` 保留实际值，包括空数组、空对象、空字符串、false 和 0，不再用默认空集合或 null 补出缺失字段。Serde 反序列化通过统一适配保留相同三态，消息持久化往返不会合并缺失与 null；`JsonFieldExt` 仅供展示映射方便地获取非空值，不参与序列化。模型必填字段缺失时可以没有整个平台投影，但不阻止保存已鉴权解码的数据；header 的可选 HTTP 头仍使用普通 `Option<String>`。

raw.payload 完整保留解码内容，包括上游放在业务内容中的字段；HTTP 头的黑名单过滤不代表对 payload 做了脱敏，例如飞书业务 JSON 内的验证 Token 仍属于完整解码内容。通用标题、正文、来源等由 `MessageContent` 映射生成，不混进平台 payload；不隐式调用外部 API。

GitHub 的模型定义集中在 `github/models.rs`。`GithubWebhook.header` 保留 event/delivery/hook_id 等白名单头；`payload` 提供 `review/thread`、`workflow/workflow_job`、`check_run/check_suite`、`release`、`discussion/answer/old_answer`、`deployment/deployment_status`、`label/assignee/milestone/changes`。Push、Create/Delete、Status 的 `ref/commits/state/context` 等字段留在 payload 根级，不再重组为 `push/git_ref/commit_status`。PR 分支引用只从原有 `head.ref/base.ref` 读取，不生成 `head_branch/base_branch`。`GithubTimestamp` 保留仓库时间原来的字符串或 Unix 秒类型；label 的字符串和对象形式也保留其原有 JSON 类型。

接收仍先验签并解码，再逐字段提取；单个可选字段类型异常不会使整个事件被拒绝或丢弃其他有效字段。字符串原样提取，不将数字/布尔值转成字符串，不裁剪文本。此次 header/payload 与 raw 存储调整明确不兼容旧消息，不提供历史消息转换，也不在读取路径维护旧格式兼容分支；新的平台模型只从当前规范的 raw 数据解析。

升级前按需备份，暂停旧实例和请求写入，由部署者物理清空 `notify_messages`（软删除不算空表）。第 3 个迁移只新增 raw_headers、删除 platform_data，不转换历史 JSON；消息表非空时拒绝迁移，不自动删除记录。Server 启动时事务性执行迁移，Workers 切换新代码前应用 0003_raw_storage.sql。需要保留旧消息时不能直接采用此升级方式；账号、设备、配置及其数据不重置。

Generic 明确是 **NoMail 自定义协议**，不是任意平台自动识别器。`GenericWebhook.header` 表达 `X-NoMail-*` 输入；`payload` 是支持的 JSON 字段、原样文本或 JSON null。JSON 分支的通用展示映射不继承文本分支的请求头。非 UTF-8 文本显式拒绝，不用有损替换伪装成可读原文。

## 使用

### Gitea

配置采用 `{"provider":"gitea","credentials":{"secret":"replace-with-a-random-secret"}}`，产品要求有效 Secret，不使用 GitHub 或 Gogs 兼容头替代 Gitea 原生签名。现有接收地址只支持 POST，可使用 JSON 或 `payload` 表单；Gitea 的 GET 投递不在当前产品接收范围内。

Gitea 使用独立模型，不复用 GitHub 类型。`metadata.event_type` 是 `X-Gitea-Event` 的事件分组，`GiteaWebhook.header.event/event_type/delivery/installation_target_type` 分别保存对应原生头信息；它们不进入 payload。Push、Create/Delete、Wiki、Status 的字段保持根级，不再包装为 `push/git_ref/wiki/commit_status`。根级 `comment` 按上游事件保留为 Wiki 文本或评论对象。其余资源保持 `issue/pull_request/review/release/package/workflow/workflow_run/workflow_job` 等字段。用户 ID 使用有符号整数，工作流 ID 使用字符串；模型按上游 v28.3.0 定义，完整内容仍在 raw。

验签始终针对 Gitea 的原始 JSON 文本：JSON 投递使用原 Body，表单投递先提取解码后的 `payload`，不能重新序列化再验签，也不能对整个编码表单验签。去重使用 `gitea:delivery` 与原生 Delivery ID；Gitea 手动 Replay 生成新 UUID，因此会作为新消息处理。Gitea 的测试投递是普通 Push，不是 GitHub Ping。

### App Store Connect

产品配置使用 `{"provider":"appstoreconnect","credentials":{"secret":"replace-with-a-random-secret"}}`，Secret 是在 App Store Connect Webhook 中设置的独立字符串，不是 API 私钥/JWT，也不是内购共享密钥。接收 POST JSON，使用原始 Body 验证 `x-apple-signature`，前缀严格为 `hmacsha256=`，不是 GitHub 的 `sha256=`。

`AppStoreConnectWebhook.payload.data` 保留官方自带的 `data` 层及其 `type/id/version/attributes/relationships`；`header` 当前为空对象。属性分别保留 `newValue/oldValue`、`newExternalBuildState/oldExternalBuildState`、`newState/oldState`、`available/territories/appId/timestamp`，不把不同事件的状态键混成同一字段。普通资源关系使用 `instance.data`，后台资源直接使用 `instance.id/type/links`，模型保留两种结构，Rust 调用方可通过 relationship 的 `reference()/self_url()` 统一访问。`marketplaceApp` 独立建模，允许某些通知没有 `instance`；version 和 timestamp 不作全局必填要求。

事件类型、版本和去重标识分别来自 `data.type`、可选的 `data.version`、`data.id`，去重 scope 为 `appstoreconnect:event`；同一资源的不同事件不会按资源 ID 合并。ID 保持字符串，不要求 UUID。反馈通知只表达资源创建，不包含可直接展示的截图、崩溃日志或完整反馈；Core 不调用 Apple API 补全这些内容。未知事件/状态与未知字段保留 raw，优先级默认 normal。

官方文档中的 `WebhookPing/webhookPings` 是管理 API 类型，不能据此推断入站 Ping 的完整契约；没有为它虚构 ACK-only 分支。有效的未知通知信封仍按普通事件处理，必须通过存储后才确认接收。该平台不是 App Store Server Notifications，不处理其 `signedPayload`/JWS。

测试将官方 `webhook-events` 的 14 个示例（12 类事件）固化为脱敏 JSON fixture，并逐字段检查类型和值；CI 不依赖外部样本、Apple 账号或联网下载。

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
        // 适配层把通用字段、event.provider、event.metadata、event.raw
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
| Stripe | 原始 Body + 时间戳签名；多个 v1 签名；Snapshot/Thin | 事件摘要；Thin 明确标记未获取详情 |
| Slack | 原始 Body + 时间戳签名；Events API、Interaction、Slash Command、URL 验证 | 文本/命令摘要；交互保留 JSON；限流通知忽略 |
| 飞书 | Verification Token；拼接 SHA256 签名；AES-256-CBC；URL 验证；v2/旧版信封 | 消息文本二次 JSON 解码与通用摘要 |
| Telegram | 固定 Secret Token；Update 类型分流 | 文本、caption、按钮 data，非文本使用 JSON 回退 |
| Shopify | Base64 HMAC；主题、API 版本、店铺、投递与事件 ID | title/name 摘要，裁剪字段使用 JSON 回退 |

事件结构采用平台专属的 `XxxWebhook { header, payload }`。Stripe 根据 payload.object 区分 Snapshot/Thin，保留 `data.object` 和 `related_object.type` 原有位置；Slack 保留 Events API 的 `event` 层、交互的 `team/user` 对象及 Slash Command 原有参数；飞书保留 payload 内原生 `header/event/sender_id`，不再生成 `content_json`。**当前只建模已支持的关键字段，不是所有上游事件的完整强类型 Schema。** 未知字段保留在 raw；未知普通事件一般使用 raw 摘要。版本保留在元数据中，并不表示已验证所有 API 版本的字段兼容性。

Telegram 的 `TelegramWebhook.payload` 直接对应 [Bot API Update 请求体](https://core.telegram.org/bots/api#update)（2026-11-10）：模型顶层为必填 `update_id` 和 29 类可选更新分支，嵌套结构遵循平台原有层级。`header` 当前为空对象，不包含 Secret Token。不再提供从嵌套对象提升的 `chat_id/message_id/callback_query_id`，也没有额外的 `update` 包装；聊天及消息 ID 从 `payload.message.chat.id/payload.message.message_id` 或相应事件路径读取，不回填历史平台数据。嵌套字段采用字段级宽容提取，不强制要求文档中的必填字段，以免一个异常字段丢弃其他信息；类型不匹配不做字符串/数字转换。联合类型按 `type/status/source` 或消息 `date` 分流，未知变体保留为未知对象。文本、caption、回调数据及字符串 ID 原样保存，实体偏移使用 UTF-16 单元；媒体只包含文件标识与元数据，不下载内容。接收端允许仅有 `update_id` 的信封，也不因缺少模型字段或存在多个更新分支而拒收已鉴权数据；未知更新及附加元数据保留在 raw，缺少有效 update_id 时不猜测去重键。该接入仅对应 Bot API，不包括 Telegram Gateway。

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
- `HeaderLookup` 的鉴权查询必须不区分请求头大小写并保留重复值，`entries()` 必须枚举全部运行时可见头（含重复和空值），不能仅枚举平台已知头。提供的 `Vec<(String, String)>` 和两端适配满足要求；轮换签名不会因覆盖同名项丢失。Server 的原始字节头捕获与原有文本鉴权查询分离，非文本值不能被有损转换成可匹配的凭据。
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
