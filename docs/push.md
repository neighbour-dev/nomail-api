# 手机系统推送

`nomail-api` 可选通过 `nomail-push` 发送 iOS APNs 和 Android FCM 提醒。未配置时，收件箱和 SSE 照常工作，不创建待发任务。

## 配置实例凭据

从官方 `nomail-push` 获取**实例 Token**，不是管理 Token，也不是手机的设备 Token。APNs 和 FCM 共用同一个实例 Token。Server 和 Workers 固定使用 `https://push.nomail.sg`；不需要配置地址。请勿将实例 Token 提交到仓库或交给 App。

- **Server / Docker Compose**：在 API 实例的 `.env` 中设置 `NOMAIL_PUSH_TOKEN=<实例 Token>`，然后重启 Server；Compose 需重建容器，让新环境变量生效。
- **Cloudflare Workers**：用 `wrangler secret put NOMAIL_PUSH_TOKEN` 保存实例 Token。应用 D1 迁移并部署 Workers；保留 `wrangler.toml` 的每分钟 Cron，用于补偿未完成的交接。

APNs 中转环境需与 App 签名环境匹配；FCM 需要在中转上配置与 Android App 对应的 Firebase 项目。

API 后台提交任务和查询状态时均发送 `Authorization: Bearer <实例 Token>`。同一 `callerId` 的 Token 轮换时，更新部署配置即可，无需清空队列或让 App 重新注册。旧 Token 会立即失效；更新配置前，未交接任务只会等待至各自的投递期限。不要在仍有待处理任务时切换到其他 `callerId` 或独立的中转数据库；中转的幂等范围不跨调用方或数据库。

## App 接口

用当前登录设备的 API access token 调用：

- `PUT /api/device/push/token` 上报或更新设备 Token。iOS 发送 `{"channel":"apns","deviceToken":"<APNs 设备 Token>"}`，Android 发送 `{"channel":"fcm","deviceToken":"<FCM 注册 Token>"}`。两字段均必填，设备由登录会话识别。API 未配置中转时返回 HTTP 503；能否投递由中转的实际渠道配置决定。
- `DELETE /api/device/push/token` 删除当前登录设备的推送 Token。关闭提醒或撤回系统权限时调用。退出、踢下线或重新登录会清除旧登记；正常刷新登录令牌不会。登录后和设备 Token 更新时须重新同步。

API 将设备 Token、渠道、消息与投递 ID，以及通知文案交给中转。标题为 `NoMail: <消息标题>`（超长时按 UTF-8 字符边界截断），正文使用收件箱里的消息摘要。消息标题和摘要会经过官方中转及 APNs／FCM，可能出现在锁屏上；原始 Webhook 和请求头不会发送。App 点开通知后可用 `messageId` 向原 API 实例读取 `GET /api/messages/{id}`；App 自身仍需实现设备注册、通知展示与跳转。

## 通知等级

`GET /api/device/push/preferences` 读取当前登录设备的通知设置，`PUT /api/device/push/preferences` 一次保存总开关和四个等级开关。两个接口均要求 API access token，设备由登录会话识别；同账号的其他设备不受影响。

```json
{"enabled":true,"low":false,"normal":true,"high":true,"urgent":true}
```

总开关默认关闭，四个等级默认开启。App 获得系统通知权限后才能登记设备 Token 并开启推送；权限被撤回时，应删除设备 Token。后端只为会话有效、已登记 Token 且总开关与消息等级都开启的设备创建推送任务。关闭推送不影响收件箱和 SSE。

设备设置在退出后保留；重新登录需要重新同步 Token。后台领取尚未交接的任务时也会检查当前开关；在任务期限内重新开启，已有待发任务仍可能发送。已交接给中转的任务无法撤回。重新开启不会补推关闭期间入库的消息。

## 投递与排查

只有首次入库的新消息会添加任务，每个已登记的有效设备一条。API 最长尝试交接十分钟；交接后由中转负责厂商重试。中转接受不等于手机收到，已接受的任务也不能撤回。短暂的授权错误会记录不含凭据的警告，不会停用手机；只有中转报告目标无效时才清理仍匹配的设备注册版本。
