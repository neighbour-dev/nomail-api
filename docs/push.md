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

- `PUT /api/auth/device/push-token` 上报或更新设备 Token。iOS 发送 `{"channel":"apns","deviceToken":"<APNs 设备 Token>"}`，Android 发送 `{"channel":"fcm","deviceToken":"<FCM 注册 Token>"}`。两字段均必填，设备由登录会话识别。API 未配置中转时返回 HTTP 503；能否投递由中转的实际渠道配置决定。
- `DELETE /api/auth/device/push-token` 删除当前登录设备的推送 Token。关闭提醒或撤回系统权限时调用。退出、踢下线或重新登录会清除旧登记；正常刷新登录令牌不会。登录后和设备 Token 更新时须重新同步。

API 保存渠道与设备 Token 并交给中转；注意保护实例数据库和备份。中转还会收到消息/投递 ID 和固定文案「NoMail：你有一条新消息」，不接收 Webhook 正文、标题或摘要。App 点开通知后可用 `messageId` 向原 API 实例读取 `GET /api/messages/{id}`；App 自身仍需实现设备注册、通知展示与跳转。

## 投递与排查

只有首次入库的新消息会添加任务，每个已登记的有效设备一条。API 最长尝试交接十分钟；交接后由中转负责厂商重试。中转接受不等于手机收到，已接受的任务也不能撤回。短暂的授权错误会记录不含凭据的警告，不会停用手机；只有中转报告目标无效时才清理仍匹配的设备注册版本。
