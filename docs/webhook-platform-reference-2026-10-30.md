# 常见平台 Webhook 接收协议与配置协议参考

首次整理：2026-10-30；配置要求及官方来源核对：2026-11-03。

覆盖 GitHub、GitLab、Stripe、Slack、飞书、Telegram、Shopify。用于实现各平台专用的 Webhook 接收、配置与解析器，不要求统一各家的业务格式。保留原文件名以免破坏已有引用。

## 1. 范围与约定

本文描述平台通过 HTTP POST 向你的服务器投递事件，不包含向群机器人地址发送消息的 Incoming Webhook，也不包含 WebSocket 接收方式。

“配置协议”指平台侧订阅参数、接收端必须持有的凭据及其条件约束，不表示各平台提供相同的配置 API，也不是 NoMail 已实现的新请求 Schema。下文的英文配置项名称用于表达语义；平台原生字段另行标明。

- **平台要求**、**接收端建议**和 **NoMail 当前实现**分别描述，不将安全建议误写成平台必填项。
- **接收凭据**用于验签、验证 Token 或解密；**管理凭据**用于创建订阅、授权或补充查询。只接收事件时，不应为了验签额外索取平台 API Token。
- 必填性取决于已选模式：例如启用加密就必须提供解密密钥；支持可选 Secret 的平台，不代表接收端应该默许缺失签名。
- 生产接收地址建议统一使用公网 HTTPS，平台硬性限制和特殊例外见各节。配置管理、授权范围与订阅创建应与接收处理分离。

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

### 2.1 配置与凭据总览

| 平台 | 接收端认证/解密配置 | 凭据来源与必填条件 | 影响接收协议的非秘密配置 |
|---|---|---|---|
| GitHub | `webhookSecret` | 用户在 Webhook 设置中填写；平台可选，接收端建议要求配置 | 安装目标、事件集合、JSON/表单编码 |
| GitLab | `authMode`、`signingToken` 或 `secretToken` | 新版 Signing Token 由 GitLab 生成；旧式 Secret Token 自定义；按模式必填 | 实例及版本、项目/组范围、触发事件、分支过滤、自定义模板 |
| Stripe | `signingSecrets` | Stripe 为各端点和环境生成的签名密钥；验签必需，轮换时可有多个 | Snapshot/Thin、事件集合、API 版本、事件来源范围、测试/正式环境 |
| Slack | `signingSecret` | 应用 Basic Information / App Credentials；请求签名验证必需 | Events/交互/Slash Commands、各自 Request URL、事件与授权范围 |
| 飞书 | `verificationToken`、`encryptKey` | 应用“事件与回调 → 加密策略”；Token 独立，Encrypt Key 在启用加密时必需 | HTTP 订阅模式、事件集合、事件版本、应用身份及权限 |
| Telegram | `secretToken` | 用户在 `setWebhook.secret_token` 中指定；平台可选，接收端建议要求配置 | Bot 身份、allowed_updates、连接数、证书、云端/自建 Bot API |
| Shopify | `appClientSecret` | 应用凭据中的 Client Secret；应用 HTTPS 投递验签必需 | 应用级/店铺级订阅、topics、API 版本、字段筛选、过滤条件 |

表中的凭据名称不能因为都叫 Secret 或带 `whsec_` 前缀而互换。特别是 GitLab 的 Signing Token 需要 Base64 解码，Stripe 的签名密钥不使用这条解码规则。

### 2.2 NoMail 当前配置契约与边界

核对位置：[公开 DTO](../crates/nomail-app/src/http/contract/config.rs)、[平台开放与认证映射](../crates/nomail-app/src/webhook/service.rs)、[配置业务](../crates/nomail-app/src/webhook/config/service.rs) 及 [OpenAPI](swagger.json)。

- 当前产品配置开放全部七个平台，并保留自定义 `generic`。平台专属配置和校验在 Core 的 `platforms/<provider>/config.rs`；App 只管理名称、描述、所有权、接收 key、启停和整体加密持久化。
- 创建请求为 `name`、`description` 和严格的 `platform: {provider, credentials}`，移除旧顶层 provider/secret 及配置 source。具体字段、示例和更新语义见[安装指南](install.md#创建和修改-webhook-配置)；不以本节语义名称直接推断字段名，例如 GitHub 使用 credentials.secret，Shopify 使用 credentials.clientSecret。
- 仅 Generic 可显式选择 type=none。其他平台要求对应凭据，飞书 verificationToken/encryptKey 独立；GitLab 显式选择签名或旧 Token，Stripe 支持 1–5 个 signingSecrets。签名时间窗口目前由接收运行时使用默认 300 秒，不是配置 API 可调参数。
- 查询只返回 platform.provider/authentication 安全摘要，凭据不回传。PATCH 省略 platform 表示保留，提供时完整替换；description 可用空字符串清除。新 SQLite/D1 基线不兼容或迁移旧配置，不自动清库。
- 本文的订阅范围、权限、管理 API 凭据、版本和过滤等平台侧选项，并不都对应 NoMail 字段或完整机器人能力；不支持的字段会被严格拒绝。平台侧订阅不会因创建 NoMail 配置自动创建。接收路径仍为 `/api/webhook/{key}`，返回相对 url 需拼接公网域名。

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

### 3.5 配置协议

**平台侧：**仓库的 **Settings → Webhooks → Add webhook**，填写 Payload URL、选择 Content type 和事件，并勾选 Active。创建仓库 Webhook 需要仓库所有者或管理员权限；组织 Webhook 需要组织所有者。GitHub App 在应用设置中配置 URL/Secret，并在 **Permissions & events** 中选择权限及事件，事件可用性受权限限制。

| 配置项 | 必填性与要求 | 接收端用途 |
|---|---|---|
| Payload URL / Webhook URL | 平台侧必填；部署时使用接收端完整 URL，建议 HTTPS | 路由到对应配置 |
| Content type | 平台支持 JSON 或表单；建议明确选择 `application/json` | 决定是否需要解析 `payload` 表单字段 |
| Events | 平台侧选择 push、全部或指定事件；建议只选所需事件 | 确定处理范围，不是凭据 |
| Secret → `webhookSecret` | 平台可选；用户生成高熵随机字符串，两端保存相同原文 | 验证 `X-Hub-Signature-256`；不是 PAT、App Private Key 或 OAuth Client Secret |
| Active | 要立即接收时启用 | 与 NoMail 自己的 enabled 状态相互独立 |
| 目标身份 | 按仓库、组织或 GitHub App 接入登记 | 用于隔离和上下文检查；不能替代验签 |

创建后 GitHub 发送 `ping`，不是要求回显 challenge 的 URL 验证。若接收端声明启用签名认证，应拒绝缺失或无效签名，不按请求头是否存在自动关闭认证。GitHub 管理 API 的访问 Token 只在自动创建/管理订阅时需要，纯接收验签不需要。

### 3.6 官方文档

- [创建 Webhook：设置、权限与事件选择](https://docs.github.com/en/webhooks/creating-webhooks)
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

### 4.5 配置协议

**平台侧：**项目或组的 **Settings → Webhooks → Add new webhook**。项目需 Maintainer 或 Owner，组需 Owner；组 Webhook 的套餐条件见官方文档。登记实例地址及实际部署版本，不能只按 GitLab.com 最新文档假定自建实例支持全部选项。

| 配置项 | 必填性与要求 | 接收端用途 |
|---|---|---|
| URL | 平台侧必填；特殊字符按要求 URL 编码 | 投递地址；生产保留 SSL 验证 |
| Trigger / 分支过滤 | 选择触发事件；push 可按全部、通配符或 RE2 正则过滤 | 决定事件和分支范围 |
| `authMode` | 接收端应显式选签名、旧式 Token 或受控迁移模式 | 固定预期认证方式，避免无条件降级 |
| Signing token → `signingToken` | 支持该功能的实例推荐；点击 Generate signing token 生成，仅展示一次，API 不返回 | 解码后用于 Standard Webhooks 验签及时间校验 |
| Secret token → `secretToken` | 旧式认证按需自定义；Token 模式必填 | 常量时间比较 `X-Gitlab-Token`，不是 Body HMAC |
| Custom headers | 平台可选，最多 20 个，不能覆盖投递头 | 只有登记了额外认证契约才使用；敏感值同样需要保护 |
| Custom webhook template | 平台可选，须输出有效 JSON | 默认解析器应禁用模板，或登记并实现对应模板版本 |

新版允许同一个 Webhook 同时配置 Signing Token 和 Secret Token，便于迁移。官方给出了按签名头存在与否回退的迁移方案；接收端若采用兼容模式，必须显式启用、限定时间并在迁移后关闭，不能让默认强认证配置永久回退。签名模式缺失签名必须失败。

平台的 Test / Recent events 可验证实际投递，没有通用 challenge 回显流程。自建实例可由管理员配置全局 mTLS 客户端证书，这是 TLS/部署配置，不是每条接收配置的 HMAC 密钥。管理订阅所用的 GitLab Access Token 与上述两个 Token 独立。

### 4.6 官方文档

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

### 5.5 配置协议

**平台侧：**Workbench 的 **Webhooks → Create an event destination**，选择事件来源、Payload 格式及事件类型，目的地选择 Webhook endpoint，填写公网 HTTPS URL。Snapshot 还需选 API 版本；Thin 和 Snapshot 应分别注册端点，不假设一个配置同时具有两种结构。

| 配置项 | 必填性与要求 | 接收端用途 |
|---|---|---|
| Endpoint URL | 已注册的公网端点必须 HTTPS；本地 CLI 转发可使用本地 HTTP | POST 接收；不要依赖 3xx 跳转 |
| Signing Secret → `signingSecrets` | 平台为端点生成，设置页 Reveal secret 获取，通常以 `whsec_` 开头；接收验签必需 | 对时间戳和原始 Body 验签，按原文使用，不去前缀后 Base64 解码 |
| 测试/正式环境及端点身份 | 应分别登记 | 即使 URL 相同，测试和正式的签名密钥也不同；不同端点不可共用假定 |
| Payload format | 选择 Snapshot 或 Thin | 决定事件 Schema 和是否需要补充查询 |
| API version | Snapshot 按端点配置版本；记录并维护兼容范围 | 不用本次 API 请求头版本覆盖历史事件结构 |
| Event types / events_from | 选择事件集合及本账号、Connected accounts 或组织范围 | 明确事件来源及处理范围，不能根据收到的请求自动扩大范围 |
| 时间容忍窗口 | 接收端策略；官方 SDK 默认为 300 秒 | 防重放并保持服务器时钟同步，不以 0 禁用检查 |

Stripe CLI `stripe listen` 输出的是该本地转发链路的签名密钥，不能混用 Dashboard 端点密钥。Roll secret 可以让旧密钥延迟最多 24 小时过期；接收端应在限定轮换窗口内接受配置的多个密钥及多个 `v1` 候选，过期后移除旧密钥。

不需要 challenge 回显，使用测试事件验证投递。单纯验签、解析 Snapshot 或保存 Thin 通知不需要 Stripe API Key；若要补查 Thin 事件或关联资源，才另外配置具有所需权限和账号上下文的 API 凭据，不能把 API Key 放进签名密钥字段。

### 5.6 官方文档

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

### 6.4 配置协议

**平台侧：**先创建 Slack App，在 **Basic Information → App Credentials** 取得 Signing Secret；三个 HTTP 功能分别配置，不能将 Events API URL 当作其他功能已经启用。

| 功能/配置项 | 必填性与平台设置 | 接收端要求 |
|---|---|---|
| Signing Secret → `signingSecret` | 来自应用后台，签名验证必需 | 三种 HTTP 功能共用该应用的签名算法；不是 OAuth Client Secret、Bot Token、App-level Token 或已弃用的 Verification Token |
| Events API | Event Subscriptions 开启，配置并验证 Request URL，选择事件 | 接收 JSON；每个应用只有一个 Events API Request URL，工作区和事件需在服务端分流 |
| Events 权限及安装 | 按事件文档申请 OAuth scopes，将应用安装/授权到目标工作区；新增权限需重新授权 | 实际事件可见性受授权和用户/Bot 可见范围限制；订阅事件不等于获得权限 |
| Interactivity | Interactivity & Shortcuts 开启并填写 Request URL | 接收表单中的 `payload` JSON；Options Load URL 为按需菜单加载的独立设置 |
| Slash Commands | 每个命令配置 Command、Request URL、Short Description；Usage Hint 等按需设置 | 接收直接表单字段；每个命令有自己的 Request URL |
| 功能类型及应用身份 | 接收端应登记允许的功能及 App，按需限制 workspace | 防止将不同回调 Schema 混为同一种事件 |

Events API 保存 URL 前需通过 `url_verification`，返回 HTTP 200 及原 challenge，不套 NoMail 的普通响应包装。Events API、交互和命令均需要快速确认，正常事件及交互/命令的响应窗口为 3 秒；动态菜单等需要特定业务响应的功能，不能仅因可解析就宣称完整支持。

Events HTTP URL 会验证 SSL 证书；公开分发的交互/命令 URL 必须 HTTPS 且不能使用自签名证书。官方允许单工作区交互 URL 使用 HTTP，命令仍建议 HTTPS；生产接收端统一建议 HTTPS。Signing Secret 可在后台 Regenerate，更新后应同步接收端；不要把文档中 **Client Secret** 的 24 小时有效期直接套给 **Signing Secret**。

只验签不需要保存 OAuth Token；安装授权、调用 Web API、发送回复是另外的能力和凭据。Socket Mode 的 App-level Token 不属于本节 HTTP 配置。

### 6.5 官方文档

- [HTTP Request URL 配置与 Challenge](https://docs.slack.dev/apis/events-api/using-http-request-urls/)
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

### 7.6 配置协议

**平台侧：**开发者后台进入应用的 **开发配置 → 事件与回调**。在 **加密策略** 中取得 Verification Token，并按需设置 Encrypt Key；在 **事件配置** 中选择 **将事件发送至开发者服务器**，填写请求地址、验证后订阅所需事件。此模式不需要为接收而配置 App Secret 建立长连接。

| 配置项 | 必填性与来源 | 接收端用途 |
|---|---|---|
| 请求地址 | HTTP 订阅必填；官方要求 IPv4 公网地址，每个应用一个事件请求地址 | 所有订阅事件投递到此处；生产建议 HTTPS |
| Verification Token → `verificationToken` | 后台默认生成的应用验证标识；采用 Token 校验时必填，建议同时保存用于 URL 验证 | 比较 URL 验证/旧版顶层 `token` 或 v2 `header.token`；不是 Encrypt Key |
| Encrypt Key → `encryptKey` | 平台可选；配置加密策略后接收端必填，值与后台一致 | 普通事件签名校验和 AES 解密；不是 App Secret，也不直接使用原文作为 AES key |
| 加密/认证策略 | 接收端应显式登记是否预期加密、是否校验 Token | 未启用加密时可采用 Token 校验；启用后普通事件按签名和加密协议处理，不因缺失 `encrypt` 自动接受明文 |
| 事件集合、版本与权限 | 根据具体事件文档选择，核对所需应用权限及生效条件 | 确定 v1/v2 等结构和实际可见范围；不能把所有事件权限列成固定通用集合 |
| App ID / 预期租户 | 应用身份可登记；租户约束按自建或多租户应用场景决定 | 解码后校验上下文，不能替代签名或 Token |

**Verification Token 和 Encrypt Key 是两个独立参数，不应共用一个 Secret 字段。**Token-only 接入必须能表达“有 verificationToken、无 encryptKey”；加密模式必须能同时保存两个不同的值，不能因为提供 Token 就隐式开启加密。

保存地址时飞书发送 challenge，请求可能明文或加密。必须在 **1 秒内**返回 `{"challenge":"原值"}`；加密请求先解密取 challenge。官方普通事件安全校验说明明确不包括请求网址校验，因此不能无条件要求 URL 验证携带与普通事件相同的签名头；应使用独立的验证路径和已配置凭据，不能借此放开普通事件认证。

重置后台 Token 或 Encrypt Key 时应同步接收端并重新验证，官方本节未给出通用双密钥宽限保证，不应编造自动轮换窗口。接收不需要 tenant_access_token；若后续调用飞书 API，再独立配置 App ID/App Secret 等管理凭据。卡片交互回调与事件订阅也需分别核对，不直接复用所有业务配置。

### 7.7 官方文档

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

### 8.5 配置协议

**平台侧：**从 BotFather 创建 Bot，使用 **Bot Token** 调用 `setWebhook` 注册 URL。Bot Token 是管理凭据，不是 `secret_token`；仅接收、验证请求时无需把 Bot Token 交给接收端。

| `setWebhook` 参数 | 必填性与官方约束 | 接收端配置含义 |
|---|---|---|
| `url` | 必填；云端 Bot API 要求 HTTPS；空字符串用于移除 Webhook | 生产配置应是完整接收地址；移除是管理动作 |
| `secret_token` → `secretToken` | 平台可选，1–256 字符，仅 `A-Z`、`a-z`、`0-9`、`_`、`-` | 建议启用，两端保存一致并校验对应请求头；不能使用带冒号的 Bot Token 代替 |
| `allowed_updates` | 可选字符串数组；不传沿用旧设置 | 显式记录事件范围；空数组不代表绝对全部，默认排除 `chat_member`、`message_reaction`、`message_reaction_count` |
| `max_connections` | 云端可选，1–100，默认 40 | 控制并发投递，是平台管理参数，不是认证信息 |
| `certificate` | 自签名证书场景需以 InputFile 上传公钥证书 | 属于 TLS 配置；不上传私钥，不把证书内容当验签 Secret |
| `ip_address` | 可选，固定投递 IP，替代 DNS 解析结果 | 与 URL/证书部署保持一致 |
| `drop_pending_updates` | 可选，一次性丢弃待投递更新 | 具有数据丢弃语义，不应作为每次更新默认开启的持久选项 |
| Bot 身份 / Bot API 模式 | 接收端应登记 Bot 标识；明确云端或自建 | 去重隔离和限制校验；不要从无认证 Body 推断可信 Bot |

云端 Webhook 支持端口 **443、80、88、8443**，端口 80 不表示可以使用明文 HTTP。自建 Local Bot API Server 可使用 HTTP、局域网 IP 和其他端口等，属于单独部署模式，不能套云端限制或默认放宽生产模式。

同一个 Bot 不能同时通过 Webhook 与 `getUpdates` 接收更新；再次 `setWebhook` 是更新该 Bot 的目标，而不是新增并行订阅。更新 `allowed_updates` 不影响调用前已产生的更新，短期仍可能收到旧范围的事件。没有 challenge 回显流程，使用 `getWebhookInfo` 检查状态和错误，并以真实测试更新验证投递。

### 8.6 官方文档

- [setWebhook 参数完整参考](https://core.telegram.org/bots/api/methods/setwebhook.md)
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

### 9.5 配置协议

**本节范围为应用 Webhook 的 HTTPS 投递。**推荐应用级订阅用 `shopify.app.toml`；需要按店铺配置时用 GraphQL Admin API。Shopify Admin 内创建的 custom app 不能使用 CLI 应用配置订阅，应走 GraphQL Admin API。不要将商家后台手工创建的通知订阅及其凭据来源未经核对直接等同于这里的应用订阅。

| 配置项 | 必填性与来源 | 接收端用途 |
|---|---|---|
| 应用级 `topics` / 店铺级 `topic` | 应用级支持主题列表；GraphQL 使用 `ORDERS_CREATE` 等枚举 | 与实际订阅的 `orders/create` 等请求头主题对应 |
| `uri` | 订阅目的地必填；本节使用 HTTPS URL | 应用级开发配置也支持相对路径，但平台最终必须能访问实际地址 |
| API version | 应用级用 `[webhooks].api_version`；店铺级由创建订阅的 Admin API URL 版本决定 | 登记预期版本，并检查实际 `X-Shopify-API-Version` |
| 应用 Client Secret → `appClientSecret` | 从应用凭据取得；应用 HTTPS 投递验签必需 | 用于 HMAC，不是 Admin API Access Token；同一应用的店铺订阅不等于各有一个独立签名 Secret |
| 店铺、应用身份与 scopes | 安装并授权对应主题要求的权限；店铺级创建订阅需该店铺 Admin API 授权 | 可限制预期 shop/app 上下文；请求头本身不等于已验签的身份声明 |
| `include_fields` / `includeFields` | 可选；未设置则发送完整主题 Payload | 可能改变解析器必需字段；应禁用裁剪或登记所需字段集合 |
| `filter`、`name` 等 | 按订阅方式和当前 API Schema 选择；name 为可选标识 | 过滤控制投递范围，不是认证条件；不能把所有可选项当作两种管理方式完全等价 |
| `compliance_topics` | Shopify App Store 分发应用须订阅规定合规主题 | 走应用配置/后台，不支持通过 Admin API 订阅；仅收件箱存储不能代替合规业务处理 |

应用级配置示例（平台配置，不是 NoMail API 请求）：

```toml
[webhooks]
api_version = "2026-10"

[[webhooks.subscriptions]]
topics = ["products/create"]
uri = "https://nomail.example.com/api/webhook/EXAMPLE_KEY"
```

配置变更需按应用发布流程生效，不是只编辑本地 TOML 就改变生产订阅。主题所需 scopes 以官方主题表为准，不能用一个固定权限覆盖全部主题。

轮换 Client Secret 后，官方说明 HMAC 切换到新密钥可能最多需要 1 小时；接收端轮换方案应考虑新旧密钥兼容及到期清理。HTTPS 建连超时为 1 秒，整个请求超时 5 秒，应快速返回 2xx，不依赖跳转。没有通用 challenge 回显流程，可用 CLI webhook trigger 和应用投递日志验证；应用级和 API 管理订阅的持续失败处理不同，不能统一假设会被自动删除。

### 9.6 官方文档

- [订阅配置、权限、API 版本与管理方式](https://shopify.dev/docs/apps/build/webhooks/subscribe/)
- [请求头、投递结构、字段筛选](https://shopify.dev/docs/apps/build/webhooks/delivery-structure)
- [HMAC 验证与去重](https://shopify.dev/docs/apps/build/webhooks/verify-deliveries)
- [全部主题及 Payload](https://shopify.dev/docs/api/webhooks)

## 10. 接收与配置接入约束

1. 按端点配置确定平台和密钥，不仅凭请求头判断可信来源。
2. 保留原始 Body，先完成需要的验签，再转换 JSON 或表单。
3. 按功能、事件类型、版本选 Schema，不用单一结构覆盖整个平台。
4. URL 验证、Ping、限流通知等单独处理。
5. 明确是否允许 GitLab 自定义模板、Shopify 字段裁剪。
6. 验证业务必需字段，同时容忍未知新增字段和文档声明的可选字段。
7. 去重、业务幂等与验签分开；签名正确不等于事件没有处理过。
8. 日志中脱敏固定 Token、密钥及敏感业务字段，原始请求按访问权限和保留期限安全存储。
9. 公共名称、来源、说明与平台专属认证字段分开；平台订阅参数、接收凭据和管理 API 凭据也分开，不把它们压缩成无类型的 `secret`。
10. 保存配置时按平台及认证模式校验条件必填项，拒绝凭据混用；切换平台不能沿用语义不兼容的旧密钥。无认证必须是明确的接收端策略，不因漏填字段或缺少签名头而自动降级。
11. 明确凭据更新、清除与轮换语义，不将省略、空字符串和显式删除混为一谈；查询只返回配置状态，不回传凭据。多密钥接受窗口要有限且可清理。
12. 创建本地接收配置、完成平台 URL 验证、授权并激活订阅是不同步骤；管理 API 返回成功也不等于已成功收到事件。记录平台侧与本地侧各自的启停状态和测试结果。

**固定解析的关键是固定“平台功能 + 订阅配置 + 事件类型 + 版本”，不是假设一家平台只有一种 Body。**
