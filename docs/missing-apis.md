# NoMail 缺失接口分析

## 1. 文档目的

本文档基于以下内容整理：

- Figma 设计稿：`Nomail-new-format-v20260817`
- 当前 Go/Fiber 路由和 Handler
- 当前 PostgreSQL/GORM 数据模型
- 单用户、自托管优先的产品决策

本文档只描述尚未实现的接口和相关业务缺口，不代表这些接口已经进入开发。

## 2. 当前已实现能力

### 2.1 认证与账号

- `POST /api/auth/login`
- `POST /api/auth/refresh-token`
- `POST /api/auth/logout`
- `GET /api/auth/devices`
- `GET /api/auth/account`
- `PATCH /api/auth/account`

### 2.2 Webhook 配置

- `GET /api/webhook/configs/`
- `POST /api/webhook/configs/`
- `GET /api/webhook/configs/:id`
- `PATCH /api/webhook/configs/:id`
- `PATCH /api/webhook/configs/:id/enabled`
- `DELETE /api/webhook/configs/:id`

### 2.3 其他

- `POST /api/webhook/:key`
- `GET /api/health`

当前 Webhook 接口只有处理流程骨架，尚未完成真实验签、消息解析、消息入库和手机推送。

## 3. P0：核心闭环接口

### 3.1 消息列表

```http
GET /api/messages
Authorization: Bearer <access-token>
```

建议查询参数：

| 参数 | 类型 | 说明 |
|---|---|---|
| `page` | integer | 页码，默认 1 |
| `pageSize` | integer | 每页数量，默认 20，最大 100 |
| `query` | string | 搜索标题和内容 |
| `status` | string | `unread`、`read` |
| `level` | string | 消息等级 |
| `webhookConfigId` | integer | 按 Webhook 配置筛选 |
| `pinned` | boolean | 是否保留 |
| `from` | date-time | 开始时间 |
| `to` | date-time | 结束时间 |

建议响应：

```json
{
  "code": 0,
  "msg": "success",
  "data": {
    "items": [
      {
        "id": "0198c000-0000-7000-8000-000000000001",
        "title": "生产服务器磁盘空间不足",
        "summary": "磁盘使用率已达到 92%",
        "source": "server-monitor",
        "level": "critical",
        "status": "unread",
        "pinned": false,
        "webhookConfigId": 1,
        "createdAt": "2026-08-17T10:00:00Z",
        "expireAt": "2026-09-16T10:00:00Z"
      }
    ],
    "total": 1,
    "page": 1,
    "pageSize": 20
  }
}
```

### 3.2 未读消息数量

```http
GET /api/messages/unread-count
Authorization: Bearer <access-token>
```

建议响应：

```json
{
  "code": 0,
  "msg": "success",
  "data": {
    "count": 12
  }
}
```

### 3.3 消息详情

```http
GET /api/messages/:id
Authorization: Bearer <access-token>
```

详情至少应包含：

- 标题和完整内容
- 来源和消息等级
- 所属 Webhook 配置
- 已读状态和读取时间
- 保留状态
- 接收时间和过期时间
- 外部详情 URL
- 是否包含原始 Payload

### 3.4 修改消息状态

```http
PATCH /api/messages/:id
Authorization: Bearer <access-token>
Content-Type: application/json
```

建议请求：

```json
{
  "status": "read",
  "pinned": true
}
```

规则：

- `status=read` 时写入 `read_at`。
- `status=unread` 时清空 `read_at`。
- `pinned=true` 的消息不参与自动过期清理。

### 3.5 删除消息

```http
DELETE /api/messages/:id
Authorization: Bearer <access-token>
```

建议继续使用软删除，与现有 GORM 模型保持一致。

### 3.6 批量消息操作

```http
POST /api/messages/batch
Authorization: Bearer <access-token>
Content-Type: application/json
```

建议请求：

```json
{
  "ids": [
    "0198c000-0000-7000-8000-000000000001",
    "0198c000-0000-7000-8000-000000000002"
  ],
  "action": "mark_read"
}
```

建议支持的 Action：

- `mark_read`
- `mark_unread`
- `pin`
- `unpin`
- `delete`

### 3.7 原始 Payload

```http
GET /api/messages/:id/raw
Authorization: Bearer <access-token>
```

建议直接返回存储的原始 Payload 和 Content-Type。客户端下载文件不需要单独的后端接口。

## 4. P0：Webhook 消息处理闭环

以下内容不一定增加公开路由，但必须完成，否则消息 API 不会产生数据。

### 4.1 当前缺口

- Webhook Secret 没有解密和使用。
- GitHub、飞书 Parser 的 `Verify` 是空实现。
- GitHub、飞书 Parser 的 `Parse` 是空实现。
- Generic Parser 未注册。
- Parse 结果没有转换为 `NotifyMessage`。
- 消息没有写入数据库。
- 原始 Payload 和 Content-Type 没有保存。
- 没有幂等和去重处理。
- 没有应用保存期限和推送策略。
- 请求 Body 和全部 Header 会输出到日志，存在敏感信息泄露风险。

### 4.2 建议处理流程

```text
接收请求
  -> 根据 Webhook Key 获取配置
  -> 解密 Secret
  -> Provider 验签
  -> Provider 解析
  -> 标准化消息等级和字段
  -> 计算 expire_at
  -> NotifyMessage 入库
  -> 判断是否推送
  -> 创建推送任务
  -> 返回接收结果
```

## 5. P0：Push 设备与推送

当前登录设备来自 Refresh Token，只能代表登录会话，不能替代 APNs/FCM Push Token。

### 5.1 注册 Push 设备

```http
POST /api/push/devices
Authorization: Bearer <access-token>
Content-Type: application/json
```

建议请求：

```json
{
  "deviceId": "iphone-15-pro",
  "platform": "ios",
  "pushToken": "apns-device-token",
  "deviceName": "iPhone 15 Pro"
}
```

### 5.2 更新 Push 设备

```http
PATCH /api/push/devices/:deviceId
Authorization: Bearer <access-token>
```

用途：

- 更新 Push Token
- 启用或禁用推送
- 更新设备名称

### 5.3 删除 Push 设备

```http
DELETE /api/push/devices/:deviceId
Authorization: Bearer <access-token>
```

### 5.4 推送任务处理

还需要内部推送任务能力：

- 根据消息等级判断是否推送。
- Push 内容只包含消息 ID、标题和必要摘要。
- 失败后重试。
- Push Token 失效时禁用对应设备。
- 支持 APNs 和 FCM。

## 6. P0：推送和保存策略

### 6.1 获取全局策略

```http
GET /api/settings/notification-policy
Authorization: Bearer <access-token>
```

### 6.2 修改全局策略

```http
PATCH /api/settings/notification-policy
Authorization: Bearer <access-token>
Content-Type: application/json
```

建议数据：

```json
{
  "pushLevels": ["critical", "warning"],
  "storeRawPayload": true,
  "retentionDays": {
    "critical": 30,
    "warning": 7,
    "normal": 3,
    "success": 3
  }
}
```

### 6.3 Webhook 配置级策略

```http
GET /api/webhook/configs/:id/notification-policy
PATCH /api/webhook/configs/:id/notification-policy
```

实现前需要明确：

- 配置级策略是否完全覆盖全局策略。
- 配置级策略是否只能收窄全局策略。
- 没有配置级策略时是否继承全局设置。

保存期限修改只影响后续收到的消息，每条消息应在创建时固化 `expire_at`。

## 7. P0：Webhook 测试消息

```http
POST /api/webhook/configs/:id/test
Authorization: Bearer <access-token>
```

测试消息应经过正式流程：

- 创建标准化消息。
- 写入 `notify_messages`。
- 根据策略决定是否推送。
- 返回消息 ID 和处理结果。

## 8. P1：设备和会话管理

单设备登录会话撤销已由 `DELETE /api/auth/devices/:deviceId` 实现。Push Token 尚无对应数据模型，待推送模块实现后补充联动清理。

### 8.1 批量移除设备

```http
POST /api/auth/devices/batch-delete
Authorization: Bearer <access-token>
Content-Type: application/json
```

建议请求：

```json
{
  "deviceIds": ["device-a", "device-b"]
}
```

### 8.2 退出全部设备

```http
POST /api/auth/logout-all
Authorization: Bearer <access-token>
```

需要明确是否保留当前设备会话。

## 9. P1：Webhook 配置增强

### 9.1 配置统计

```http
GET /api/webhook/configs/:id/stats
Authorization: Bearer <access-token>
```

建议返回：

- 最近收到消息时间
- 今日和本月消息数量
- 未读数量
- 最近一次处理结果
- 最近一次验签或解析错误

### 9.2 轮换 Webhook Key

```http
POST /api/webhook/configs/:id/rotate-key
Authorization: Bearer <access-token>
```

Key 轮换后旧地址立即失效。实现前需要明确是否提供短暂的双 Key 过渡期。

### 9.3 清空可选字段

当前更新接口忽略空字符串，因此无法清空：

- `source`
- `secret`
- `description`

需要调整更新请求的字段表达方式，例如使用指针区分“未提供”和“主动清空”，不一定需要新增路由。

## 10. P1：实例信息

App 输入自托管地址时，需要判断目标服务是否为兼容的 NoMail 实例。

```http
GET /api/instance
```

建议响应：

```json
{
  "code": 0,
  "msg": "success",
  "data": {
    "name": "NoMail",
    "version": "1.0.0",
    "initialized": true,
    "authMode": "password"
  }
}
```

当前初始账号由部署环境变量创建，不建议增加公网账号初始化接口。

## 11. P2：数据导出

### 11.1 创建导出任务

```http
POST /api/exports
Authorization: Bearer <access-token>
```

建议请求：

```json
{
  "formats": ["json", "csv"],
  "contents": ["account", "messages"],
  "zip": true,
  "encrypted": true
}
```

### 11.2 查询导出任务

```http
GET /api/exports/:id
Authorization: Bearer <access-token>
```

### 11.3 下载导出文件

```http
GET /api/exports/:id/download
Authorization: Bearer <access-token>
```

大型导出应采用异步任务和短期有效的下载文件。

## 12. 不需要后端接口的功能

以下功能可以由 App 本地完成：

- 保存和切换自托管服务器地址
- 密码显示和隐藏
- 密码强度实时展示
- 搜索输入防抖
- 列表滑动和多选状态
- 复制通知内容、Webhook URL、Key 和 JSON
- 原始 JSON 语法高亮
- 将已获取的 JSON 保存到本地文件
- 外部 URL WebView
- 外部链接“不再提示”的本地偏好
- 提示音试听
- 每台设备独立的提示音选择
- 系统通知权限申请和打开系统设置

如果提示音要求跨设备同步，则需要额外的设备级设置接口。

## 13. 暂不建议实现的设计稿功能

以下设计与当前单用户、自托管决策冲突，建议先调整设计稿：

- 邮箱注册和邮箱登录
- App 在线初始化管理员账号
- 邮件找回密码
- Apple 登录
- 邮箱验证码登录
- NoMail Cloud 服务选择
- 多用户、团队、邀请和角色权限

当前账号策略：

- 部署时通过环境变量创建唯一账号。
- 登录后可以修改用户名和密码。
- 忘记凭据时通过服务器本地命令重置。

## 14. 实现前需要统一的产品定义

### 14.1 消息等级

设计稿中存在多套名称：

- `critical`
- `warning`
- `alert`
- `normal`
- `info`
- `success`
- `pass`

建议在实现筛选、推送和保存策略前统一枚举。

### 14.2 服务实体名称

设计稿中混用了：

- Service
- Project
- Webhook

当前后端实体名是 `WebhookConfig`。需要确定对用户展示的统一产品名称。

### 14.3 全局和配置级策略

设计稿同时存在全局通知策略和单个服务的推送策略，需要定义覆盖关系。

### 14.4 保留语义

需要明确 `pinned=true` 是永久保留、取消自动清理，还是只延长保存时间。

### 14.5 设备与会话

需要明确：

- 是否允许删除当前设备。
- 删除设备是否同时退出登录。
- 修改密码是否立即使 Access Token 失效。
- 登录设备和 Push 设备是否使用同一个设备 ID。

## 15. 推荐实施顺序

1. 完成 Webhook 验签、解析和消息入库。
2. 实现消息列表、详情、已读、保留、删除和批量操作。
3. 实现 Push Token 注册、推送任务和失败重试。
4. 实现全局推送与保存策略。
5. 实现 Webhook 测试消息。
6. 实现设备移除和全部退出。
7. 实现配置统计和 Key 轮换。
8. 最后实现数据导出等辅助能力。
