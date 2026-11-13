# Mailflare

独立实现 Mailflare 的出站通知协议，不引入、复制或链接上游代码，不调用邮件 API。
核对快照：`hieunc229/mailflare@6811563ec40b3a04f445d7a214c5c3d2c015cc10`。

- [通知发送、签名、重试与测试载荷](https://github.com/hieunc229/mailflare/blob/6811563ec40b3a04f445d7a214c5c3d2c015cc10/src/lib/email/webhooks.ts)
- [收信载荷](https://github.com/hieunc229/mailflare/blob/6811563ec40b3a04f445d7a214c5c3d2c015cc10/src/lib/email/inbound.ts)
- [发信及失败载荷](https://github.com/hieunc229/mailflare/blob/6811563ec40b3a04f445d7a214c5c3d2c015cc10/src/lib/email/send.ts)

`X-Email-Platform-Signature` 是原始 Body 字节的 HMAC-SHA256 十六进制值，
无前缀。只接受单个签名头及非空 HMAC 配置；不使用其他平台的兼容签名。
事件类型来自已验签的 Body `type`，不信任描述性的事件头。

Core 从 `X-Email-Platform-Delivery` 提取去重标识，重试次数不参与标识；
App/存储层在接收配置范围内执行去重。缺少 Delivery ID 时不生成去重标识，
测试载荷固定的 `messageId: "test"` 不用于去重。
Delivery/Attempt/Event 头不参与签名，也没有签名时间戳；不能将去重视为完整的
防重放机制。签名头在所有平台的 raw 捕获中均被过滤。

保留原生信封和字段路径，区分缺失、null、实际值；类型异常的字段只从投影省略，
完整内容仍在 raw。未知事件仍可接收，非对象载荷不生成平台投影。
收信主题作为标题，正文展示邮件元数据；失败通知映射为 high 优先级。
上游事件不含邮件正文或附件，不联网补全，也不构造未经协议提供的邮件链接。

`fixtures/database_samples.json` 的 4 个样本来自本地导出库中原以 Generic 接收的
Mailflare 收信通知。只保留载荷层级、字段类型、数字和头名称；地址、主题、消息/
线程/投递 ID 及其他文本已替换，原签名及其他请求头未导出。测试使用独立的合成
密钥重新签名，覆盖 Core 解析、Server/Workers 持久化、详情投影和重试去重，
不读取原数据库、生产凭据或真实签名。
