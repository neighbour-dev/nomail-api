# App Store Connect

本目录只实现 App Store Connect HTTP Webhook 通知协议，不实现管理 Webhook 的 REST API，也不处理内购/订阅的 App Store Server Notifications（`signedPayload` / JWS）。

## 官方来源与核对范围

2026-11-16 核对以下 Apple 官方文档；当前事件枚举列出 12 类事件，事件文章包含 14 个 JSON 示例。脱敏样例固化于 `fixtures/documented_events.json`，测试不需要 Apple 账号或联网。

- [事件定义和载荷示例](https://developer.apple.com/documentation/appstoreconnectapi/webhook-events)
- [可订阅事件枚举](https://developer.apple.com/documentation/appstoreconnectapi/webhookeventtype)
- [Webhook 配置、测试与验签](https://developer.apple.com/documentation/appstoreconnectapi/configuring-webhook-notifications)

这是官方示例覆盖和宽容读取投影，不是 Apple 全部资源 API 的模型，也不是所有历史或未来载荷的严格 Schema。没有新增未被官方载荷证明存在的 App 名称、版本号、反馈正文、截图或崩溃日志字段。

## 事件与内容映射

订阅配置使用官方大写枚举，入站 `data.type` 使用下表的原生名称，不互相改写。

| 入站事件 | 场景 | 用于摘要的原生字段 |
|---|---|---|
| `appStoreVersionAppVersionStateUpdated` | App 版本审核/发布状态变化 | `oldValue → newValue` |
| `buildBetaDetailExternalBuildStateUpdated` | 外部 TestFlight 构建状态变化 | `oldExternalBuildState → newExternalBuildState` |
| `buildUploadStateUpdated` | 构建上传处理状态变化 | `oldState → newState`，允许仅提供新状态 |
| `betaFeedbackScreenshotSubmissionCreated` | TestFlight 截图反馈创建 | 无正文，摘要留空 |
| `betaFeedbackCrashSubmissionCreated` | TestFlight 崩溃反馈创建 | 无正文，摘要留空 |
| `backgroundAssetVersionStateUpdated` | Apple 托管后台资源处理状态变化 | `oldState → newState` |
| `backgroundAssetVersionInternalBetaReleaseCreated` | 后台资源内部测试发布创建 | 无状态内容，摘要留空 |
| `backgroundAssetVersionExternalBetaReleaseStateUpdated` | 后台资源外部测试审核/可用状态变化 | `oldState → newState` |
| `backgroundAssetVersionAppStoreReleaseStateUpdated` | 后台资源 App Store 审核/分发状态变化 | `oldState → newState` |
| `alternativeDistributionPackageVersionCreated` | 替代分发包版本创建 | 无版本详情，摘要留空 |
| `alternativeDistributionPackageAvailableUpdated` | 替代分发包可用性变化 | `available` 与 `territories` |
| `alternativeDistributionTerritoryAvailabilityUpdated` | 替代分发地区可用性变化 | `available` 与 `territories`，使用独立事件标题 |

标题明确标识事件；状态字符串保留官方原值并允许未来值，不按状态自动提升关注优先级，默认 `normal`。新旧状态只读取该事件对应的字段，不跨字段猜测。摘要按 Core 公共规则合并空白并限制为 30 个 Unicode 字符；创建类通知的资源 ID 不充当摘要，也不使用整段 JSON 回退。

`available=false` 与缺失/null 不同；只有地区列表时仅输出地区，不推断可用或不可用。地区保持原顺序、重复值和字符串，不擅自翻译或补全。数组含非字符串元素时，整个类型化 `territories` 字段省略，完整值仍在 raw，避免静默过滤后呈现不完整的分发范围。

## 模型、资源关系与链接

- 保留 `data.type/id/version/attributes/relationships` 原生层级；可选字段区分缺失、显式 null 和实际值。
- `timestamp` 不强制存在；后台资源示例没有 `version`，构建上传示例没有 `timestamp` 和旧状态，均正常接收。
- 普通事件的资源引用位于 `relationships.instance.data`；后台资源事件直接在 `relationships.instance` 提供 `id/type/links`。`reference()` 统一访问资源引用，但序列化保留原始形状。
- 替代分发事件可能只有 `relationships.marketplaceApp`，不要求 `instance`。
- 消息链接优先使用 instance 的非空 `links.self`，再取嵌套引用的非空 `links.self`，最后回退 marketplaceApp。链接来自实际载荷，不构造控制台地址、不调用 API。Apple 提供的链接可能是需要认证的 API 地址，不保证可直接在浏览器查看。
- 未知事件、未知字段和类型异常内容保留在 raw；个别可选字段异常不影响其他有效字段的投影。模型无法生成时仍保留已验签解码的事件。

## 认证、去重与边界

- 使用独立 Webhook Secret，对原始 JSON Body 验证唯一的 `x-apple-signature: hmacsha256=<64 位十六进制 HMAC-SHA256>`。不使用 App Store Connect API 私钥或内购共享密钥，不接受其他平台的备用签名头。
- 签名头不保存到 raw。载荷重新序列化后不能代替原始 Body 验签。
- 去重候选为 `data.id`，scope 为 `appstoreconnect:event`；由 App 在接收配置范围内原子入库。不能用资源 ID 去重，因为同一资源可产生多次状态变化。缺少事件 ID 时不猜测去重键。
- 此协议验签不包含独立签名时间戳，不把去重或业务 `timestamp` 描述为完整的防重放保护。
- TestFlight 反馈只通知资源创建；完整反馈、截图和崩溃日志需要另行调用 Apple API，Core 不联网补全。
- 管理 API 的 `WebhookPing` 对象不作为入站 Ping 信封的依据；不凭名字丢弃通知或推断特殊 ACK。HTTP 响应包装由 App 负责。

## 测试

覆盖官方 14 个样例的逐字段投影、12 类事件的标题/摘要/链接、官方签名向量、原始字节验签、重复签名头拒绝、三态字段、异常地区集合、未来状态、备用链接、事件 ID 去重候选及 raw 重建。样例覆盖不等于穷举验证所有 Apple 事件状态和历史版本。

从仓库根目录按贡献指南先生成 OpenAPI，再运行：

```bash
cargo test --locked -p nomail-core --all-features app_store_connect
```
