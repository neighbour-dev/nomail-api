# Uptime Kuma

接收 Uptime Kuma 默认 Webhook，保留 `{ heartbeat, monitor, msg }` 原生结构。不调用 Kuma API、不执行监控检查，也不接收 UptimeRobot 等其他服务的协议。

## 来源

独立实现协议投影，不复制或引入上游代码，不增加依赖。上游许可证为 MIT；核对快照为 `louislam/uptime-kuma@1e3e2eee51821c453329ee8d7145be0568712863`。

- [Webhook 发送与自定义请求头](https://github.com/louislam/uptime-kuma/blob/1e3e2eee51821c453329ee8d7145be0568712863/server/notification-providers/webhook.js)
- [Heartbeat 字段与状态定义](https://github.com/louislam/uptime-kuma/blob/1e3e2eee51821c453329ee8d7145be0568712863/server/model/heartbeat.js)
- [Monitor 投影、通知发送与时间字段](https://github.com/louislam/uptime-kuma/blob/1e3e2eee51821c453329ee8d7145be0568712863/server/model/monitor.js)
- [上游许可证](https://github.com/louislam/uptime-kuma/blob/1e3e2eee51821c453329ee8d7145be0568712863/LICENSE)

这是固定快照的常用字段读取投影，不是所有监控类型、历史版本的完整 Schema。样例是依据源码形状独立编写的合成数据，不是官方样例或真实用户投递。

## 接入配置

通过 NoMail 接收配置 API 创建：

```json
{
  "name": "Uptime Kuma",
  "platform": {
    "provider": "uptimekuma",
    "credentials": {}
  }
}
```

使用返回的接收 URL，在 Uptime Kuma 中新增 **Webhook** 通知：

1. URL 填入 NoMail 返回的接收地址。
2. 使用 **POST**、默认 **JSON** 请求体（不是 `form-data` 或自定义模板）。
3. 无需配置认证请求头。发送测试通知，将此通知渠道关联到需要的监控项。

Uptime Kuma 默认 Webhook 不生成签名，也不要求认证。本适配器按默认协议接收，平台配置使用空 `credentials` 对象，公开认证模式为 `none`。Uptime Kuma 支持自定义请求头，但当前 NoMail 适配器不提供这些请求头的认证校验；如需认证，可在反向代理层配置。这不影响 NoMail 配置管理和收件箱 API 本身的登录认证。

本适配器不支持 GET 查询参数、multipart `form-data` 或任意自定义模板的专属解码。如使用自定义模板，需将输出调整为 NoMail Generic 协议支持的格式，并创建对应的 Generic 接收配置；仅切换 provider 并不保证原模板兼容。

## 内容与边界

- `heartbeat.status`：`0 → down`、`1 → up`、`2 → pending`、`3 → maintenance`；未知、缺失或非整数状态按 `notification` 接收，不把 `"0"` 转成数字。原值保留在模型或 raw。
- 标题包含监控名称和状态；摘要优先使用 `msg`，其次 `heartbeat.msg`，按公共规则合并空白、最多 30 个 Unicode 字符。没有文本时留空，不使用整段 JSON 回退。
- `down/up` 表示当前状态，不凭 `up` 推断此前必有故障；关注优先级默认 `normal`，不把结果直接等同于用户关注优先级。
- 链接仅使用实际 `monitor.url`，不猜测控制台地址、不联网补全。非 HTTP 监控可能没有链接。
- 测试通知、证书到期等消息可能提供 `heartbeat=null`、`monitor=null`，仍保存为普通通知，不全部误判为测试请求。
- 默认载荷没有平台提供的唯一事件或投递标识，因此本适配器无法可靠去重，不生成去重键；重复投递会分别入库。monitor ID、时间及内容哈希不作为投递身份，避免吞掉周期性提醒或后续故障。
- 已建模且类型匹配的字段保持原生层级，并保留缺失、null 和实际值的区别；类型不匹配的字段不进入类型化投影，但不影响其他有效字段。不强制要求监控或时间信息，完整解码内容（包括未知字段和类型不匹配的字段）仍保留在 raw。
- **本适配器不校验来源，没有 Body 签名、时间窗口或防重放保证。** 未在外部限制访问时，知道接收 URL 的人可发送通知；应通过内网、网络访问控制或反向代理自行限制访问，使用 HTTPS 并避免泄露接收 URL。
- 通用敏感请求头仍按全局黑名单过滤，但 **payload 不自动脱敏**。Monitor URL、诊断消息、响应正文及自定义字段可能含敏感内容，应检查上游数据并限制访问。

## 测试

Core 覆盖故障（down）、上线（up）、测试与证书通知、未知状态、数字/字符串边界、字段三态、raw 重建及无需认证的原生接收行为。Server/Workers 使用同一组合成载荷验证配置保存、无认证接收、详情、raw 和重复投递分别入库。
