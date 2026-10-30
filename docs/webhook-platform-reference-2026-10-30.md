# 常见平台 Webhook 接收协议参考

整理日期：2026-10-30。

覆盖 GitHub、GitLab、Stripe、Slack、飞书、Telegram、Shopify。用于实现各平台专用的 Webhook 接收与解析器，不要求统一各家的业务格式。

## 1. 范围与约定

本文描述平台通过 HTTP POST 向你的服务器投递事件，不包含向群机器人地址发送消息的 Incoming Webhook，也不包含 WebSocket 接收方式。

- **加密**隐藏 Body 内容；**签名**验证来源和内容完整性。没有应用层加密不代表没有 HTTPS 传输加密。
- 以下仅列出与认证、解析、去重相关的主要请求头，不穷举全部 HTTP 头。
- **Body 示例是字段节选，不是完整 Schema。**完整字段、必填性和可空性见各节官方事件文档。
- HTTP 请求头名称不区分大小写。
- `raw_body` 是收到的原始请求体字节，不能用解析后重新序列化的 JSON 替代。
- 一个平台通常有多种固定事件结构，应按功能、配置、事件类型和版本选择解析器。

## 2. 总览

| 平台 | 请求体格式 | 分流依据 | 应用层 Body 加密 | 来源验证 |
|---|---|---|---|---|
| GitHub | JSON 或表单中的 JSON | `X-GitHub-Event`、可选 `action` | 无标准加密包装 | HMAC-SHA256 |
| GitLab | JSON；支持自定义模板 | `X-Gitlab-Event`、默认 Body 的 `object_kind` | 无标准加密包装 | 新版签名 Token；旧版固定 Token |
| Stripe | Snapshot 或 Thin JSON | 端点配置、`type`、版本 | 无标准加密包装 | 带时间戳的 HMAC-SHA256 |
| Slack | JSON 或 URL 编码表单 | 功能、外层 `type`、内层事件类型 | 无标准加密包装 | 带时间戳的 HMAC-SHA256 |
| 飞书 | JSON；加密时为 `encrypt` 包装 | 解密后的请求种类、版本、事件类型 | 可选 AES-256-CBC | SHA256 拼接签名或 Verification Token |
| Telegram | JSON `Update` | 顶层更新字段 | 普通 Update 无统一加密包装 | 可选固定 Secret Token |
| Shopify | JSON 主题 Payload | Topic、API Version、订阅配置 | 无标准加密包装 | HMAC-SHA256 |

## 3. GitHub

### 3.1 请求头

| 请求头 | 用途 |
|---|---|
| `Content-Type` | `application/json` 或 `application/x-www-form-urlencoded` |
| `X-GitHub-Event` | 事件名，例如 `push`、`issues`、`pull_request`、`ping` |
| `X-GitHub-Delivery` | 投递标识，用于去重、追踪 |
| `X-GitHub-Hook-ID` | Webhook 配置 ID |
| `X-Hub-Signature-256` | 配置 Secret 后发送的 SHA256 签名 |
| `X-Hub-Signature` | 旧 SHA1 签名，不建议新接入使用 |
| `X-GitHub-Hook-Installation-Target-Type` | Webhook 安装目标类型 |
| `X-GitHub-Hook-Installation-Target-ID` | 安装目标 ID |
| `User-Agent` | 前缀为 `GitHub-Hookshot/` |

### 3.2 加密与验签

没有标准的应用层 Body 加密包装。配置 Webhook Secret 后：

```text
expected = "sha256=" + hex(HMAC-SHA256(secret, raw_body))
```

与 `X-Hub-Signature-256` 做常量时间比较。签名不含时间戳，需要投递去重及业务幂等防止重复处理。

### 3.3 Body

`X-GitHub-Event: issues`、JSON 模式示例：

```json
{
  "action": "opened",
  "issue": { "id": 123, "number": 42, "title": "Example issue" },
  "repository": { "id": 456, "full_name": "owner/repo" },
  "sender": { "login": "alice" }
}
```

表单模式形如：

```text
payload=%7B%22action%22%3A%22opened%22%2C...%7D
```

先用整个原始表单 Body 验签，再解析表单，最后将 `payload` 字段作为 JSON 解析。

### 3.4 解析规则

```text
X-GitHub-Event
  ├─ issues       → action 对应的 Issues Schema
  ├─ pull_request → action 对应的 PR Schema
  ├─ push         → Push Schema
  └─ ping         → Ping Schema
```

不是所有事件都有 `action`、`repository` 或 `sender`，需按对应事件文档判断。

### 3.5 官方文档

- [请求头、事件及完整 Payload 字段](https://docs.github.com/en/webhooks/webhook-events-and-payloads)
- [签名验证](https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries)

## 4. GitLab

**版本注意：**当前官方文档提供基于 Standard Webhooks 的签名机制。旧版自建实例可能只有 `X-Gitlab-Token`，不能假设全部实例都支持新版签名头。

### 4.1 请求头

| 请求头 | 用途 |
|---|---|
| `Content-Type` | 默认 JSON 请求体 |
| `X-Gitlab-Event` | 例如 `Push Hook`、`Merge Request Hook` |
| `X-Gitlab-Instance` | 来源实例地址 |
| `X-Gitlab-Token` | 配置旧式 Secret Token 后发送的固定值 |
| `webhook-id` | 新版消息 ID，重试保持一致 |
| `webhook-timestamp` | 新版请求生成时间，Unix 秒 |
| `webhook-signature` | 配置 Signing Token 后发送的签名列表 |
| `Idempotency-Key` | 兼容幂等标识；当前文档建议优先用 `webhook-id` |
| `X-Gitlab-Event-UUID` | 事件追踪标识；递归触发的 Webhook 可能共享该值 |
| `X-Gitlab-Webhook-UUID` | Webhook 标识，不应替代消息去重键 |
| `User-Agent` | 包含 GitLab 版本信息 |

另外支持配置自定义请求头。

### 4.2 加密与验签

没有标准的应用层 Body 加密包装。

旧式 Secret Token：比较 `X-Gitlab-Token` 与配置值。**这是固定 Token，不是 Body 签名。**

新版 Signing Token：

```text
key = Base64Decode(移除 Signing Token 的 "whsec_" 前缀)
message = webhook-id + "." + webhook-timestamp + "." + raw_body
expected = "v1," + Base64(HMAC-SHA256(key, message))
```

`webhook-signature` 可以包含多个空格分隔的签名，应匹配受支持版本的候选签名，并验证时间戳时效。

生产环境按端点配置强制要求预期认证方式，不要因为签名头缺失就无条件降级为弱认证。

### 4.3 Body

默认 Merge Request 事件示例：

```json
{
  "object_kind": "merge_request",
  "event_type": "merge_request",
  "user": { "id": 1, "name": "Alice" },
  "project": { "id": 456, "name": "demo" },
  "object_attributes": {
    "id": 123,
    "iid": 42,
    "title": "Example merge request",
    "action": "open"
  }
}
```

**GitLab 支持自定义 Payload 模板。**配置后可能变成：

```json
{ "event": "merge_request", "project_name": "demo" }
```

因此不能在未知配置下假设 `object_kind` 一定存在。

### 4.4 解析规则

```text
实例版本 + 是否启用自定义模板
  ├─ 默认 Payload → X-Gitlab-Event → 对应事件 Schema
  └─ 自定义模板   → 按该 Webhook 的模板契约解析
```

需要固定解析器时，应禁止自定义 Body 模板，或者登记模板版本。

### 4.5 官方文档

- [配置、请求头、签名及自定义模板](https://docs.gitlab.com/user/project/integrations/webhooks/)
- [所有事件及 JSON Payload](https://docs.gitlab.com/user/project/integrations/webhook_events/)

## 5. Stripe

### 5.1 请求头

| 请求头 | 用途 |
|---|---|
| `Content-Type` | `application/json` |
| `Stripe-Signature` | 时间戳和一个或多个签名 |

```http
Stripe-Signature: t=1793354400,v1=HEX_SIGNATURE
```

事件类型从 Body 读取。

### 5.2 加密与验签

没有标准的应用层 Body 加密包装。

```text
signed_payload = t + "." + raw_body
expected = hex(HMAC-SHA256(endpoint_signing_secret, signed_payload))
```

与请求头的 `v1` 比较，并检查时间戳时效。官方 SDK 默认容忍窗口为 5 分钟。

- Secret 是端点的 Webhook Signing Secret，通常以 `whsec_` 开头，不是 API Key。
- **不要照搬 GitLab 的密钥 Base64 解码规则。**
- 密钥轮换期间可能出现多个 `v1`，不能把同名项覆盖丢失。

### 5.3 Body

#### Snapshot

```json
{
  "id": "evt_123",
  "object": "event",
  "api_version": "2024-06-20",
  "created": 1710000000,
  "type": "payment_intent.succeeded",
  "livemode": false,
  "data": {
    "object": {
      "id": "pi_123",
      "object": "payment_intent",
      "amount": 2000,
      "currency": "usd",
      "status": "succeeded"
    }
  }
}
```

按端点 Snapshot 配置、API 版本和 `type` 解析 `data.object`。部分更新事件有 `data.previous_attributes`。

#### Thin

Thin 只携带元数据及关联资源标识，不能假设有 `data.object`。

```json
{
  "id": "evt_123",
  "object": "v2.core.event",
  "type": "v2.core.account.updated",
  "livemode": false,
  "created": "2026-03-09T13:00:28.435Z",
  "context": null,
  "related_object": {
    "id": "acct_123",
    "type": "v2.core.account",
    "url": "/v2/core/accounts/acct_123"
  }
}
```

按需调用 API 获取完整事件或关联资源。不同 Thin 事件的可用字段不同，不能强制要求每个事件都有相同的关联对象。

### 5.4 解析规则

```text
端点配置
  ├─ Snapshot → type + api_version → data.object Schema
  └─ Thin     → EventNotification → type → 按需 API 补充查询
```

Snapshot 示例的 `created` 是 Unix 秒数字，Thin 示例的 `created` 是时间字符串，不可共用同一字段类型假设。

去重使用事件 `id`；重试时签名及签名时间戳会重新生成。

### 5.5 官方文档

- [Webhook 接收与签名验证](https://docs.stripe.com/webhooks)
- [Snapshot、Thin 格式及版本规则](https://docs.stripe.com/events/how-events-work)
- [Snapshot Event 对象](https://docs.stripe.com/api/events/object)
- [Snapshot 事件类型](https://docs.stripe.com/api/events/types)
- [Thin 事件类型](https://docs.stripe.com/api/v2/core/events/event-types)

## 6. Slack

Slack 的 Events API、交互请求、Slash Commands 使用不同 Body 编码，不能全部当作 JSON。

### 6.1 请求头

| 请求头 | 用途 |
|---|---|
| `Content-Type` | JSON 或 URL 编码表单 |
| `X-Slack-Signature` | `v0=...` 请求签名 |
| `X-Slack-Request-Timestamp` | Unix 秒签名时间戳 |
| `X-Slack-Retry-Num` | Events API 重试次数 |
| `X-Slack-Retry-Reason` | Events API 重试原因 |

### 6.2 加密与验签

没有标准的应用层 Body 加密包装。

```text
message = "v0:" + timestamp + ":" + raw_body
expected = "v0=" + hex(HMAC-SHA256(signing_secret, message))
```

与 `X-Slack-Signature` 比较。官方示例要求时间与当前时间相差不超过 5 分钟。

无论 JSON 还是表单，都对原始 Body 验签，而不是对解析后的 `payload` 验签。

### 6.3 Body

#### Events API：JSON

```json
{
  "type": "event_callback",
  "team_id": "T123",
  "api_app_id": "A123",
  "event_id": "Ev123",
  "event_time": 1710000000,
  "event": {
    "type": "message",
    "user": "U123",
    "channel": "C123",
    "text": "hello",
    "ts": "1710000000.000001"
  }
}
```

```text
body.type
  ├─ event_callback  → event.type + 可选 subtype
  ├─ url_verification → URL 验证
  └─ app_rate_limited → 限流通知
```

不能假设所有请求都有 `event`。

URL 验证请求：

```json
{
  "type": "url_verification",
  "token": "legacy_token",
  "challenge": "challenge_value"
}
```

验证来源后按官方要求返回 `challenge`。

#### 交互：表单中的 JSON

```http
Content-Type: application/x-www-form-urlencoded
```

```text
payload=%7B%22type%22%3A%22block_actions%22%2C...%7D
```

解析表单后，再解析 `payload`：

```json
{
  "type": "block_actions",
  "user": { "id": "U123" },
  "actions": [{ "action_id": "approve", "value": "order_123" }]
}
```

#### Slash Commands：直接表单字段

```text
team_id=T123&user_id=U123&command=%2Fexample&text=hello&...
```

不是 `payload` JSON 包装，直接按表单字段解析。

### 6.4 官方文档

- [请求签名验证](https://docs.slack.dev/authentication/verifying-requests-from-slack/)
- [Events API 外层结构](https://docs.slack.dev/apis/events-api/)
- [事件类型参考](https://docs.slack.dev/reference/events/)
- [交互请求处理](https://docs.slack.dev/interactivity/handling-user-interaction/)
- [Slash Commands](https://docs.slack.dev/interactivity/implementing-slash-commands/)

## 7. 飞书

本节以 HTTP 事件订阅和 v2.0 事件结构为主。旧版事件及卡片交互等回调订阅不能直接套用同一业务 Schema。

### 7.1 请求头

| 请求头 | 用途 |
|---|---|
| `Content-Type` | JSON，例如 `application/json;charset=utf-8` |
| `X-Lark-Request-Timestamp` | 签名时间戳 |
| `X-Lark-Request-Nonce` | 签名随机值 |
| `X-Lark-Signature` | 请求签名 |

启用 Encrypt Key 时，普通事件推送应按官方规则验签。URL 验证请求走单独流程，不应假设与普通事件完全相同。

### 7.2 签名验证

```text
expected = hex(SHA256(
  UTF8(timestamp + nonce + encrypt_key) + raw_body
))
```

与 `X-Lark-Signature` 比较。

- 这是 SHA256 拼接摘要，**不是 HMAC-SHA256**。
- 字段之间没有额外分隔符。
- 启用加密后，使用加密外层的原始 Body 验签，不是解密后的 JSON。

### 7.3 Body 加密

启用 Encrypt Key 时收到：

```json
{ "encrypt": "BASE64_ENCODED_DATA" }
```

解密步骤：

```text
key        = SHA256(UTF8(encrypt_key)) 的 32 字节摘要
buffer     = Base64Decode(encrypt)
iv         = buffer 前 16 字节
ciphertext = buffer 剩余字节
plaintext  = AES-256-CBC 解密并移除 PKCS#7 填充
```

将明文按 UTF-8 JSON 解析。未启用 Encrypt Key 时收到明文 JSON，可按官方机制验证 Verification Token，但固定 Token 不等同于 Body 完整性签名。

### 7.4 解密后的 Body

#### URL 验证

```json
{
  "type": "url_verification",
  "token": "verification_token",
  "challenge": "challenge_value"
}
```

校验后返回：

```json
{ "challenge": "challenge_value" }
```

启用加密时，验证请求也可能先通过 `encrypt` 包装到达。

#### v2.0 事件

```json
{
  "schema": "2.0",
  "header": {
    "event_id": "event_123",
    "event_type": "im.message.receive_v1",
    "create_time": "1710000000000",
    "token": "verification_token",
    "app_id": "cli_123",
    "tenant_key": "tenant_123"
  },
  "event": {
    "message": {
      "message_id": "om_123",
      "message_type": "text",
      "content": "{\"text\":\"hello\"}"
    }
  }
}
```

### 7.5 解析规则

```text
加密配置 → 按需验签、解密
  ├─ URL 验证 → challenge 流程
  ├─ v2.0 → schema + header.event_type → event Schema
  └─ 旧版 → 对应旧版事件 Schema
```

- `header.create_time` 是毫秒时间戳字符串。
- 消息 `content` 是字符串，需根据 `message_type` 使用对应内容结构；文本示例需要再次解析 JSON。
- 旧版事件不能强制要求存在 `header`。

### 7.6 官方文档

- [HTTP 订阅配置、URL 验证、加密原理](https://open.feishu.cn/document/event-subscription-guide/event-subscriptions/event-subscription-configure-/choose-a-subscription-mode/send-notifications-to-developers-server?lang=zh-CN)
- [接收事件：验签与解密](https://open.feishu.cn/document/server-docs/event-subscription-guide/event-subscription-configure-/encrypt-key-encryption-configuration-case?lang=zh-CN)
- [接收消息事件及字段](https://open.feishu.cn/document/uAjLw4CM/ukTMukTMukTM/reference/im-v1/message/events/receive)

## 8. Telegram Bot API

### 8.1 请求头

| 请求头 | 用途 |
|---|---|
| `Content-Type` | JSON 请求体 |
| `X-Telegram-Bot-Api-Secret-Token` | `setWebhook` 配置 `secret_token` 后，每次请求携带该值 |

普通 Bot API Webhook 没有标准的逐请求 Body HMAC 签名头。

### 8.2 加密与认证

普通 Update 不使用统一的应用层加密包装。

```text
X-Telegram-Bot-Api-Secret-Token == setWebhook 配置的 secret_token
```

这是固定 Token，不是 Body 签名，也不是 Bot Token。

### 8.3 Body

直接发送一个 `Update`，不是 API 响应的 `{"ok":true,"result":...}` 包装。

消息示例：

```json
{
  "update_id": 123456,
  "message": {
    "message_id": 42,
    "from": { "id": 10001, "is_bot": false, "first_name": "Alice" },
    "chat": { "id": 10001, "type": "private" },
    "date": 1710000000,
    "text": "hello"
  }
}
```

按钮回调示例：

```json
{
  "update_id": 123457,
  "callback_query": {
    "id": "query_123",
    "from": { "id": 10001, "is_bot": false, "first_name": "Alice" },
    "chat_instance": "instance_123",
    "data": "approve"
  }
}
```

### 8.4 解析规则

`update_id` 是公共标识；其余顶层可选更新字段，官方规定每个 Update 最多出现一个。

```text
message         → Message
edited_message  → Message
channel_post    → Message
callback_query  → CallbackQuery
inline_query    → InlineQuery
my_chat_member  → ChatMemberUpdated
其他字段        → Update 文档对应类型
```

即使存在 `message` 也不一定存在 `text`，消息可能是图片、贴纸或服务消息。去重键应包含 Bot 身份和 `update_id`。

### 8.5 官方文档

- [setWebhook、Secret Token 与投递规则](https://core.telegram.org/bots/api#setwebhook)
- [Update 完整字段列表](https://core.telegram.org/bots/api/types/update.md)
- [Message 对象](https://core.telegram.org/bots/api/types/message.md)
- [CallbackQuery 对象](https://core.telegram.org/bots/api/types/callbackquery.md)

## 9. Shopify

仅描述 HTTPS 投递，不包含 Amazon EventBridge 或 Google Cloud Pub/Sub。

### 9.1 请求头

| 请求头 | 用途 |
|---|---|
| `Content-Type` | `application/json` |
| `X-Shopify-Topic` | 例如 `products/update`、`orders/create` |
| `X-Shopify-Hmac-Sha256` | Base64 编码的 HMAC-SHA256 签名 |
| `X-Shopify-Shop-Domain` | 来源店铺域名 |
| `X-Shopify-API-Version` | 实际用于序列化 Payload 的 API 版本 |
| `X-Shopify-Webhook-Id` | 投递标识，用于投递去重 |
| `X-Shopify-Event-Id` | 同一商家操作产生的多次投递共享的事件标识 |
| `X-Shopify-Triggered-At` | 触发时间 |
| `X-Shopify-Name` | 可选订阅名称 |

### 9.2 加密与验签

没有标准的应用层 Body 加密包装。

```text
expected = Base64(HMAC-SHA256(app_client_secret, raw_body))
```

与 `X-Shopify-Hmac-Sha256` 比较。密钥是应用 Client Secret，不是店铺 Access Token。

签名覆盖 Body，不包含 Topic、版本等请求头，也不包含签名时间戳。应使用 HTTPS，并通过去重及业务幂等处理重复请求。

### 9.3 Body

`products/update` 示例：

```json
{
  "id": 9554194432293,
  "title": "T-Shirt",
  "status": "active",
  "vendor": "My Store",
  "updated_at": "2025-04-22T14:30:00-05:00",
  "variants": [{ "id": 123456789, "price": "29.99", "sku": "TSHIRT-001" }]
}
```

通常直接发送主题对应的资源或专用对象，没有统一的 `event`、`data` 包装。

### 9.4 解析规则

```text
X-Shopify-Topic + X-Shopify-API-Version + 订阅字段筛选配置
  → 对应 Payload Schema
```

支持通过 `include_fields`／`includeFields` 筛选字段，因此同一主题可能只收到：

```json
{
  "id": 9554194432293,
  "variants": [{ "id": 123456789, "price": "29.99" }]
}
```

如果解析器依赖完整资源，应禁止字段裁剪，或登记必需字段。

- 删除类、合规类主题不一定发送完整资源。
- 金额字段经常是字符串，不应统一当数字解析。
- 按实际收到的 API Version 校验兼容性，不要只相信本地预期版本。

### 9.5 官方文档

- [请求头、投递结构、字段筛选](https://shopify.dev/docs/apps/build/webhooks/delivery-structure)
- [HMAC 验证与去重](https://shopify.dev/docs/apps/build/webhooks/verify-deliveries)
- [全部主题及 Payload](https://shopify.dev/docs/api/webhooks)

## 10. 解析器接入约束

1. 按端点配置确定平台和密钥，不仅凭请求头判断可信来源。
2. 保留原始 Body，先完成需要的验签，再转换 JSON 或表单。
3. 按功能、事件类型、版本选 Schema，不用单一结构覆盖整个平台。
4. URL 验证、Ping、限流通知等单独处理。
5. 明确是否允许 GitLab 自定义模板、Shopify 字段裁剪。
6. 验证业务必需字段，同时容忍未知新增字段和文档声明的可选字段。
7. 去重、业务幂等与验签分开；签名正确不等于事件没有处理过。
8. 日志中脱敏固定 Token、密钥及敏感业务字段，原始请求按访问权限和保留期限安全存储。

**固定解析的关键是固定“平台功能 + 订阅配置 + 事件类型 + 版本”，不是假设一家平台只有一种 Body。**
