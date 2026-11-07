# 常见平台 Webhook 接收协议与配置协议参考

覆盖 GitHub、GitLab、Stripe、Slack、飞书、Telegram、Shopify 的 HTTP POST 事件投递，不包含向机器人地址发送消息的 Incoming Webhook、WebSocket 或消息总线投递。

## 1. 通用约定

- **配置协议**指平台侧订阅参数、接收凭据及条件约束，不表示各平台具有统一配置 API。
- **接收凭据**用于验签、Token 校验或解密；**管理凭据**用于创建订阅、授权或调用 API，两者不可混用。仅接收验签通常不需要管理凭据。
- `raw_body` 是原始请求体字节，不能用解析后重新序列化的 JSON 替代。公式中的文本前缀按 UTF-8 编码，再与原始字节拼接；`hex` 表示小写十六进制。
- HTTP 请求头名称不区分大小写；签名和固定 Token 使用常量时间比较。按端点配置确定预期认证方式，不因缺少认证头自动降级。
- 平台可选的 Secret 不等于接收端应接受无认证请求。生产地址建议使用公网 HTTPS；应用层加密与 HTTPS 传输加密是不同机制。
- 按功能、订阅配置、事件类型和版本选择 Schema。下文 Body 均为字段节选，完整字段及可选性以官方事件文档为准。
- 验签、去重和业务幂等分别处理；去重键需隔离平台、端点或账号。除非平台明确保证，否则不依赖投递顺序。
- 普通事件快速确认接收，耗时业务异步处理；URL 验证和需业务响应的交互请求按专用协议响应。

## 2. 总览

| 平台 | Body / 分流依据 | 来源验证与接收凭据 | 应用层 Body 加密 |
|---|---|---|---|
| GitHub | JSON 或表单中的 JSON；`X-GitHub-Event`、可选 `action` | Webhook Secret，HMAC-SHA256 | 无标准包装 |
| GitLab | JSON，可自定义模板；`X-Gitlab-Event` | 新版 Signing Token，HMAC-SHA256；旧版固定 Secret Token | 无标准包装 |
| Stripe | Snapshot / Thin JSON；端点格式、`type`、版本 | 端点 Signing Secret，带时间戳 HMAC-SHA256 | 无标准包装 |
| Slack | JSON 或表单；功能、外层 `type`、内层事件类型 | 应用 Signing Secret，带时间戳 HMAC-SHA256 | 无标准包装 |
| 飞书 | JSON；请求种类、事件版本及类型 | Encrypt Key 拼接 SHA256 签名；或 Verification Token | 可选 AES-256-CBC |
| Telegram | JSON `Update`；顶层更新字段 | 可选固定 `secret_token`，无标准逐请求 Body 签名 | 普通 Update 无统一包装 |
| Shopify | JSON 主题 Payload；Topic、API Version、字段筛选 | 应用 Client Secret，HMAC-SHA256 | 无标准包装 |

## 3. 平台详情

### 3.1 GitHub

#### 配置

在仓库或组织的 **Settings → Webhooks** 配置；GitHub App 在应用设置中配置。

| 配置项 | 要求 |
|---|---|
| Payload URL / Webhook URL | 完整接收地址，建议 HTTPS |
| Content type | `application/json` 或 `application/x-www-form-urlencoded`，建议 JSON |
| Events / Active | 选择所需事件并启用；App 事件可用性受应用权限限制 |
| Secret | 平台可选，接收端建议要求配置；用户生成高熵随机字符串，两端保存相同原文 |

#### 接收与验证

| 请求头 | 用途 |
|---|---|
| `X-GitHub-Event` | 事件名，如 `issues`、`push`、`ping` |
| `X-GitHub-Delivery` | 投递标识，用于去重与追踪 |
| `X-Hub-Signature-256` | 配置 Secret 后发送的签名 |

```text
expected = "sha256=" + hex(HMAC-SHA256(secret, raw_body))
```

与 `X-Hub-Signature-256` 比较。旧 `X-Hub-Signature` 使用 SHA1，不建议新接入使用。签名不含时间戳。

`issues` 事件示例：

```json
{
  "action": "opened",
  "issue": { "id": 123, "number": 42, "title": "Example issue" },
  "repository": { "id": 456, "full_name": "owner/repo" }
}
```

表单模式先对整个原始 Body 验签，再解析表单，将 `payload` 字段作为 JSON 解析。按事件名及可选 `action` 分流；并非所有事件都有 `action`、`repository` 或 `sender`。

创建后会发送 `ping`，无需回显 challenge。Webhook Secret 不是 PAT、App Private Key 或 OAuth Client Secret。

**官方文档：**[创建与配置](https://docs.github.com/en/webhooks/creating-webhooks) · [事件与请求头](https://docs.github.com/en/webhooks/webhook-events-and-payloads) · [验签](https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries)

### 3.2 GitLab

#### 配置

在项目或组的 **Settings → Webhooks** 配置。项目需要 Maintainer / Owner，组需要 Owner；功能受实例版本和套餐影响。

| 配置项 | 要求 |
|---|---|
| URL / Trigger | 填写接收地址，选择事件及可选分支过滤；生产保留 SSL 验证 |
| Signing token | 支持该功能的实例推荐使用；由 GitLab 生成，仅展示一次，API 不返回 |
| Secret token | 旧式固定 Token，由用户配置；旧实例可能仅支持此方式 |
| Custom webhook template | 可选，改变 Body 结构；固定解析器应禁用模板，或明确模板契约 |

#### 接收与验证

| 请求头 | 用途 |
|---|---|
| `X-Gitlab-Event` | 事件名，如 `Push Hook`、`Merge Request Hook` |
| `X-Gitlab-Instance` | 来源实例地址 |
| `webhook-id` | 新版消息 ID，重试保持一致；优先用作去重标识 |
| `webhook-timestamp` | 新版请求生成时间，Unix 秒 |
| `webhook-signature` | 配置 Signing Token 后发送的签名列表 |
| `X-Gitlab-Token` | 配置旧式 Secret Token 后发送的固定值 |

新版遵循 Standard Webhooks：

```text
key = Base64Decode(移除 signing_token 的 "whsec_" 前缀)
message = webhook-id + "." + webhook-timestamp + "." + raw_body
expected = "v1," + Base64(HMAC-SHA256(key, message))
```

匹配 `webhook-signature` 中以空格分隔的受支持签名，并检查时间戳时效。旧式认证只比较 `X-Gitlab-Token`，不验证 Body 完整性。两种 Token 可同时配置用于迁移，但接收端兼容回退应显式启用，迁移后关闭。

默认 Merge Request Body：

```json
{
  "object_kind": "merge_request",
  "project": { "id": 456, "name": "demo" },
  "object_attributes": { "iid": 42, "title": "Example MR", "action": "open" }
}
```

默认按 `X-Gitlab-Event` 和对应事件结构解析；自定义模板不能假定包含 `object_kind`。旧实例可用 `Idempotency-Key` 去重；`X-Gitlab-Event-UUID`、`X-Gitlab-Webhook-UUID` 不应直接替代消息去重键。无通用 challenge 回显流程。

**官方文档：**[配置、认证与请求头](https://docs.gitlab.com/user/project/integrations/webhooks/) · [事件结构](https://docs.gitlab.com/user/project/integrations/webhook_events/)

### 3.3 Stripe

#### 配置

在 **Workbench → Webhooks** 创建事件目的地，选择 Webhook endpoint。

| 配置项 | 要求 |
|---|---|
| Endpoint URL | 注册端点必须为公网 HTTPS；本地 CLI 转发可用 HTTP |
| Payload format | Snapshot 或 Thin，分别注册端点 |
| Event types / 来源范围 | 选择事件及本账号、Connected accounts 或组织等来源范围 |
| API version | Snapshot 按端点配置版本解析 |
| Signing Secret | 平台为各端点及测试/正式环境分别生成，通常以 `whsec_` 开头；不是 API Key |

#### 接收与验证

Body 为 JSON，签名头格式如下：

```http
Stripe-Signature: t=1793354400,v1=HEX_SIGNATURE
```

```text
message = t + "." + raw_body
expected = hex(HMAC-SHA256(signing_secret, message))
```

与任一 `v1` 候选匹配，并检查时间戳；官方 SDK 默认容忍窗口为 300 秒。Secret 按原文使用，**不采用 GitLab 的去前缀、Base64 解码规则**。

| 格式 | 主要结构与解析依据 |
|---|---|
| Snapshot | `id`、`type`、`api_version`、`data.object`；部分更新事件包含 `data.previous_attributes` |
| Thin | 事件通知元数据及按事件提供的关联资源标识；不能假定包含 `data.object` 或固定的关联对象 |

Snapshot 节选：

```json
{
  "id": "evt_123",
  "type": "payment_intent.succeeded",
  "api_version": "2024-06-20",
  "data": { "object": { "id": "pi_123", "status": "succeeded" } }
}
```

按格式及 `type` 分流，不强行统一字段类型：Snapshot 的 `created` 为 Unix 秒数字，Thin 通知中为时间字符串。补查 Thin 完整事件或资源才需要 API 凭据。

去重使用事件 `id`；重试会重新生成签名和签名时间戳。密钥轮换期间可能有多个 `v1`，旧密钥可延迟最多 24 小时过期。CLI `stripe listen` 密钥与 Dashboard 注册端点密钥不能混用。无需回显 challenge，正常接收快速返回 2xx，不依赖重定向。

**官方文档：**[配置与验签](https://docs.stripe.com/webhooks) · [Snapshot / Thin](https://docs.stripe.com/events/how-events-work) · [Snapshot Event](https://docs.stripe.com/api/events/object) · [Thin 事件](https://docs.stripe.com/api/v2/core/events/event-types)

### 3.4 Slack

#### 配置

创建 Slack App，从 **Basic Information → App Credentials** 取得 Signing Secret。它不是 OAuth Client Secret、Bot Token、App-level Token 或旧 Verification Token。

| 功能 | 平台配置 | Body 编码 |
|---|---|---|
| Events API | Event Subscriptions 开启，验证 Request URL，选择事件并授予所需 scopes | JSON |
| Interactivity | Interactivity & Shortcuts 开启并填写 Request URL | URL 编码表单，`payload` 字段内为 JSON |
| Slash Commands | 为每个命令配置 Request URL 等参数 | URL 编码表单，直接包含 `command`、`text` 等字段 |

三类功能分别配置，不能因配置 Events URL 就假定其他功能已启用。事件可见性受应用安装授权和用户 / Bot 可见范围限制。生产统一建议使用有效证书的 HTTPS 地址。

#### 接收与验证

```text
timestamp = X-Slack-Request-Timestamp
message = "v0:" + timestamp + ":" + raw_body
expected = "v0=" + hex(HMAC-SHA256(signing_secret, message))
```

与 `X-Slack-Signature` 比较，并检查时间与当前时间相差不超过 5 分钟。表单请求也对整个原始 Body 验签，不是对解析后的 `payload` 验签。

Events API 节选：

```json
{
  "type": "event_callback",
  "event_id": "Ev123",
  "team_id": "T123",
  "event": { "type": "message", "text": "hello" }
}
```

先按外层 `type` 分流：`event_callback` 再按 `event.type` 及可选 `subtype` 解析；`url_verification`、`app_rate_limited` 不一定有 `event`。

URL 验证在校验来源后，返回 HTTP 200 及原 `challenge`，例如 `{"challenge":"原值"}`。正常事件、交互和命令通常需在 3 秒内确认；动态菜单等按专用协议返回业务内容。Events API 用 `event_id` 去重；`X-Slack-Retry-Num`、`X-Slack-Retry-Reason` 描述重试，不是事件 ID。

**官方文档：**[验签](https://docs.slack.dev/authentication/verifying-requests-from-slack/) · [Events URL 与验证](https://docs.slack.dev/apis/events-api/using-http-request-urls/) · [事件结构](https://docs.slack.dev/apis/events-api/) · [交互](https://docs.slack.dev/interactivity/handling-user-interaction/) · [Slash Commands](https://docs.slack.dev/interactivity/implementing-slash-commands/)

### 3.5 飞书

本节描述 HTTP 事件订阅，以 v2.0 事件为主；旧事件、卡片交互回调需分别核对 Schema。

#### 配置

在应用 **开发配置 → 事件与回调** 中配置加密策略，选择 **将事件发送至开发者服务器** 并订阅事件。

| 配置项 | 要求 |
|---|---|
| 请求地址 | 每个应用一个事件请求地址，需 IPv4 公网可达；生产建议 HTTPS |
| Verification Token | 后台生成的应用验证标识，用于 Token 校验和 URL 验证 |
| Encrypt Key | 平台可选；启用后接收端必须配置相同值，用于签名和解密 |
| 事件、版本及权限 | 按具体事件文档选择，并核对应用权限与生效条件 |

Verification Token 与 Encrypt Key 独立，均不是 App Secret。未配置 Encrypt Key 时为明文 JSON，可校验顶层 `token` 或 v2 的 `header.token`；固定 Token 不提供 Body 完整性签名。

#### 签名与解密

启用 Encrypt Key 后，普通事件用以下请求头验签：`X-Lark-Request-Timestamp`、`X-Lark-Request-Nonce`、`X-Lark-Signature`。

```text
expected = hex(SHA256(UTF8(timestamp + nonce + encrypt_key) + raw_body))
```

与 `X-Lark-Signature` 比较。这是拼接 SHA256，**不是 HMAC**，字段之间无额外分隔符；签名覆盖加密外层原始 Body。

收到 `{"encrypt":"BASE64_ENCODED_DATA"}` 后：

```text
key        = SHA256(UTF8(encrypt_key)) 的 32 字节摘要
buffer     = Base64Decode(encrypt)
iv         = buffer 前 16 字节
ciphertext = buffer 剩余字节
plaintext  = AES-256-CBC 解密并移除 PKCS#7 填充
```

#### 事件与 URL 验证

解密后按 UTF-8 JSON 解析。v2.0 事件节选：

```json
{
  "schema": "2.0",
  "header": {
    "event_id": "event_123",
    "event_type": "im.message.receive_v1",
    "create_time": "1710000000000",
    "token": "verification_token"
  },
  "event": { "message": { "message_type": "text", "content": "{\"text\":\"hello\"}" } }
}
```

按 `schema`、`header.event_type` 分流，以 `header.event_id` 去重。`create_time` 是毫秒时间戳字符串；消息 `content` 是字符串，需按 `message_type` 进一步解析。旧版不能强制要求 `header`。

保存 URL 时收到 `{"type":"url_verification","token":"…","challenge":"…"}`；启用加密时先解密。校验后在 **1 秒内**返回 `{"challenge":"原值"}`。URL 验证不适用普通事件的完整签名头要求，应单独处理，不能因此放宽普通事件认证或接受非预期明文。

**官方文档：**[配置、URL 验证与加密原理](https://open.feishu.cn/document/event-subscription-guide/event-subscriptions/event-subscription-configure-/choose-a-subscription-mode/send-notifications-to-developers-server?lang=zh-CN) · [验签与解密](https://open.feishu.cn/document/server-docs/event-subscription-guide/event-subscription-configure-/encrypt-key-encryption-configuration-case?lang=zh-CN) · [接收消息事件](https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/im-v1/message/events/receive)

### 3.6 Telegram Bot API

#### 配置

用 Bot Token 调用 `setWebhook`；Bot Token 是管理凭据，不是接收认证用的 `secret_token`。

| `setWebhook` 参数 | 要求 |
|---|---|
| `url` | 云端 Bot API 要求 HTTPS；空字符串用于移除 Webhook |
| `secret_token` | 平台可选，建议启用；1–256 字符，仅允许 `A-Z`、`a-z`、`0-9`、`_`、`-` |
| `allowed_updates` | 可选事件数组；省略沿用旧值，空数组默认仍排除 `chat_member`、`message_reaction`、`message_reaction_count` |
| `max_connections` | 云端 1–100，默认 40 |
| `certificate` | 自签名场景上传公钥证书，不上传私钥 |
| `drop_pending_updates` | 可选，一次性丢弃待投递更新，有数据丢弃语义 |

云端支持端口 443、80、88、8443，端口 80 仍要求 HTTPS。自建 Local Bot API Server 的 HTTP、地址及端口限制不同，需单独核对。

#### 接收与验证

校验 `X-Telegram-Bot-Api-Secret-Token` 与配置值一致。这是固定 Token，不是 Body HMAC，也不是 Bot Token。

Body 直接为 JSON `Update`，没有 API 响应的 `ok` / `result` 包装：

```json
{
  "update_id": 123456,
  "message": {
    "message_id": 42,
    "chat": { "id": 10001, "type": "private" },
    "date": 1710000000,
    "text": "hello"
  }
}
```

`update_id` 之外的顶层可选更新字段最多出现一个；按 `message`、`edited_message`、`callback_query` 等分流。消息可能是图片、贴纸或服务消息，不能要求总有 `text`。去重键包含 Bot 身份及 `update_id`。

同一 Bot 的 Webhook 与 `getUpdates` 互斥；再次 `setWebhook` 会更新目标，不是新增并行订阅。修改 `allowed_updates` 不影响已产生更新。无需回显 challenge，正常接收返回 2xx，可用 `getWebhookInfo` 检查状态。

**官方文档：**[setWebhook](https://core.telegram.org/bots/api#setwebhook) · [Update](https://core.telegram.org/bots/api/types/update.md) · [Message](https://core.telegram.org/bots/api/types/message.md) · [CallbackQuery](https://core.telegram.org/bots/api/types/callbackquery.md)

### 3.7 Shopify

本节仅描述**应用 Webhook 的 HTTPS 投递**；商家后台手工创建的通知订阅及其密钥来源需另行核对。

#### 配置

应用级订阅使用 `shopify.app.toml`；按店铺配置的订阅使用 GraphQL Admin API。Shopify Admin 内创建的 custom app 使用后者。

| 配置项 | 要求 |
|---|---|
| `topics` / `topic` | 选择主题；应用配置如 `orders/create`，GraphQL 枚举如 `ORDERS_CREATE` |
| `uri` | HTTPS 投递地址；应用级开发配置可用相对路径 |
| API version | 应用级用 `[webhooks].api_version`；店铺级由创建订阅的 Admin API URL 版本决定 |
| 应用 Client Secret | 应用 HTTPS 投递的 HMAC 密钥，不是店铺 Access Token |
| 权限与安装 | 按主题要求授予 scopes 并安装授权；不使用通用固定权限集合 |
| `include_fields` / `includeFields`、`filter` | 可选；字段裁剪改变 Body 结构，过滤改变投递范围 |

#### 接收与验证

| 请求头 | 用途 |
|---|---|
| `X-Shopify-Topic` | 主题，如 `products/update` |
| `X-Shopify-API-Version` | 实际序列化版本 |
| `X-Shopify-Shop-Domain` | 来源店铺域名 |
| `X-Shopify-Hmac-Sha256` | Base64 编码签名 |
| `X-Shopify-Webhook-Id` | 投递去重标识 |
| `X-Shopify-Event-Id` | 关联同一商家操作产生的多次投递 |

```text
expected = Base64(HMAC-SHA256(app_client_secret, raw_body))
```

与 `X-Shopify-Hmac-Sha256` 比较。签名不覆盖 Topic、版本、店铺等请求头，也不含签名时间戳；不能将这些请求头单独当作已验签的身份声明。轮换 Client Secret 后，切换至新密钥生成 HMAC 可能最多需要 1 小时。

Body 通常直接为主题资源或专用对象，没有统一 `event` / `data` 包装：

```json
{ "id": 9554194432293, "title": "T-Shirt", "variants": [{ "price": "29.99" }] }
```

按 Topic、实际 API Version 及字段筛选配置解析。删除、合规主题不一定包含完整资源；金额常为字符串。解析器依赖的字段不能被订阅配置裁剪。

无需回显 challenge。建连超时为 1 秒，整个请求超时 5 秒，快速返回 2xx，3xx 也视为失败。App Store 分发应用须另行满足合规主题订阅及处理要求，不能用普通事件存储代替。

**官方文档：**[订阅配置](https://shopify.dev/docs/apps/build/webhooks/subscribe/) · [请求头与结构](https://shopify.dev/docs/apps/build/webhooks/delivery-structure) · [验签与去重](https://shopify.dev/docs/apps/build/webhooks/verify-deliveries) · [主题与 Payload](https://shopify.dev/docs/api/webhooks)
