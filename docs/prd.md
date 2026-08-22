PRD
---
1. 产品概述

1.1 产品名称
NoMail

1.2 产品定位
NoMail 是一个云托管优先的个人通知收件箱，用于聚合服务器、监控告警、GitHub、SaaS 平台、自定义脚本等来源的通知消息，并统一推送到手机 App。用户可以在 App 内集中查看通知历史、详情、来源、等级和状态。
NoMail 不定位为即时通讯工具，也不试图替代飞书、Telegram、企业微信、Slack 等 IM 平台。它解决的是系统事件通知分散、容易错过、缺乏短期回看能力的问题。
商业化策略：提供免费版和付费订阅版，免费版可满足个人基础使用，付费版解锁更多消息量、更长时间保存、自定义规则等高级功能。同时，为满足自托管和隐私需求，后期将提供功能适当裁剪的自建版本（开源或商业授权），与云托管版形成差异化。

1.3 一句话介绍
NoMail：把服务器、GitHub 和 SaaS 通知统一收进一个 App，云端或自建，随你选择。

1.4 Slogan
你的云原生通知收件箱。

或：
集中接收，及时提醒，看完即清。


---
2. 背景与问题（保持不变，略）

2.1 当前问题
在个人开发者、独立开发者、自托管和轻量运维场景中，很多系统都会产生通知，例如：
- 服务器状态通知
- 监控告警
- GitHub / GitLab 通知
- CI/CD 构建结果
- Docker 容器状态
- 数据库备份结果
- SSL 证书过期提醒
- 云服务资源异常
- SaaS 平台事件通知
- 自定义脚本和定时任务通知
  这些通知常被发送到飞书、Telegram、企业微信、邮件或微信群中。但这些平台本质上是聊天/协作工具，不是专门的通知收件箱。

2.2 使用 IM 平台接收系统通知的问题
1. 通知和聊天混在一起  系统消息容易被聊天内容淹没。
2. 历史回看不方便  用户想查找某个服务今天是否告警、某次 GitHub 构建是否失败时，需要在 IM 聊天记录中搜索，体验较差。
3. 通知价值周期短  大部分通知只在短期内有价值，看过之后通常不需要长期保存。
4. 实时通知不可靠  手机 Push 可能因为静音、系统折叠、网络问题、App 后台限制等原因被错过。
5. 不需要完整 IM 能力  用户只是想接收、查看、回顾通知，不需要群聊、组织架构、审批、会议、文档等复杂功能。

2.3 产品机会
NoMail 提供一个独立于 IM 的轻量通知中心：
消息先可靠接收并短期保存，再根据重要程度决定是否推送到手机，用户可以在 App 内集中查看，并通过过期策略自动清理低价值通知。

---

3. 产品目标

3.1 核心目标
1. 聚合多来源系统通知。
2. 避免通知分散在飞书、Telegram、邮件等平台中。
3. 保证用户即使错过手机 Push，也可以在 App 内回看历史。
4. 支持短期保存与自动清理，避免通知堆积。
5. 保持产品轻量，不做完整 IM 或企业告警平台。
6. 提供可持续的商业模式：通过免费增值（Freemium）让用户快速上手，付费升级获取更高容量和高级能力，同时为自建版预留付费/开源路径。

3.2 非目标
NoMail 第一阶段不做以下能力：
- 多用户团队协作
- 群聊和即时通讯
- 审批流
- 工单系统
- 复杂值班排班
- 企业级告警升级
- 复杂权限管理
- 独立 Web 管理端
- 长期知识库归档
- 完整日志分析平台
- 替代 Grafana、Sentry、PagerDuty 等专业平台
- 第一阶段不提供团队协作、多用户共享（但支持同一账户多设备）。


---

4. 目标用户

4.1 主要用户
- 个人开发者：拥有个人服务器、博客、GitHub 项目、Docker 服务、定时任务，希望统一接收各类系统通知。
- 独立开发者：维护多个小型产品、SaaS、小程序或自动化任务，需要及时知道异常事件，但不愿意部署和维护后端服务（云托管版更合适）。
- 小团队技术负责人：需要集中接收关键告警和项目事件，但按个人工具使用 NoMail，不依赖团队协作。
- 家庭服务器 / NAS 用户：需要接收磁盘空间、备份任务、容器状态等通知，可能希望自建以保护隐私。
- 新增：希望免运维的用户：不想自己搭建服务器、配置数据库、处理证书和升级，直接注册即用云托管版本。

4.2 用户特征
- 有一定技术能力，能够配置 Webhook 或编写脚本调用 API。
- 有多个通知来源。
- 不希望通知散落在 IM 或邮箱中。
- 对数据隐私有不同偏好：部分用户可接受云端，部分用户坚持自建。
- 希望通知可以短期回看，但不需要永久保存。


---

5. 核心使用场景

5.1 服务器告警
用户的服务器 CPU、内存、磁盘或服务状态异常时，监控系统调用 NoMail Webhook。NoMail 保存消息，根据等级判断是否推送到手机。用户点击通知后进入 App 查看详情。
5.2 GitHub 构建失败
GitHub Actions 构建失败后，通过 Webhook 或脚本调用 NoMail。NoMail 将其标记为 GitHub 来源、warning 等级，并推送到手机。用户之后可在 App 内查看失败记录。
5.3 定时任务完成或失败
用户的备份脚本、爬虫任务、数据同步任务完成或失败时，发送消息到 NoMail。成功类消息可以只入库不 Push，失败类消息立即 Push。
5.4 SaaS 平台通知聚合
多个 SaaS 平台产生事件，例如支付失败、订单异常、接口报错、账单提醒等，都可以通过统一 Webhook 发送到 NoMail。
5.5 错过通知后的回看
用户当时没有看到手机 Push，之后打开 NoMail App，可以看到未读消息、严重告警、某个来源的历史通知。


---

6. 产品核心逻辑（基本不变，略）

6.1 原始逻辑
早期设想为：
消息来了 → 入库 → 推送 → 分类 → 查看 → 搜索 → 归档
6.2 调整后的合理逻辑
结合真实通知场景，更合理的逻辑是：
消息来了 → 接收校验 → 短期入库 → 判断是否推送 → 分类/聚合/降噪 → App 查看 → 自动过期/手动保留
进一步简化为：
接收 → 缓存 → 提醒 → 回看 → 清理
6.3 关键判断
1. 入库是必要的  手机 Push 不是可靠存储，用户可能错过通知。因此消息必须先保存到后端。
2. 入库不等于永久保存  大部分通知生命周期很短，只需要保存 3 天、7 天、30 天等。
3. 不是每条消息都应该 Push  成功类、普通类消息可以只入库；失败类、告警类消息才需要推送。
4. 归档不是核心，自动过期才是核心  通知不是知识库，更多时候应根据生命周期自动清理。、
5.


---

7. 功能范围

7.1 MVP 功能（云托管 SaaS 版）

云托管版作为首发形态，所有功能均包含在免费和付费订阅中，具体配额见第 15 章“商业化与订阅模式”。

7.1.1 用户账户体系
- 支持邮箱注册、登录。
- 支持密码找回。
- 同一账户可登录多个设备，消息和设备绑定。
- 后端提供 JWT 或 Session 管理。

7.1.2 项目（Webhook）管理
- 创建多个通知项目（免费版限制数量，付费版放开）。
- 每个项目生成唯一 Webhook 地址（URL）和 Webhook Key（用于鉴权）。
- 支持启用/禁用项目。
- 支持编辑项目名称和默认策略（等级阈值、保存天数等）。
- 项目列表在 App 内可查看。

7.1.3 Webhook/API 接收通知
- 外部系统通过 HTTP POST 向 /api/webhook/{webhook_key} 发送通知。
- 支持标准 JSON 消息格式（包含 title, content, level, source, project, url, tags 等）。
- 也支持简单文本格式（仅发送 content，自动生成 title）。
- 接收后进行格式校验、鉴权（通过 Webhook Key 识别项目）。
- 限流（每个项目每分钟最多 60 条，每小时 1000 条，超过返回 429）。

7.1.4 消息短期入库
- 保存标准化后的消息，包含来源、标题、内容、等级、状态、创建时间、过期时间。
- 关联到项目 ID。
- 原始 payload 可选存储（用于调试）。
- 默认保存策略：根据等级设置过期时间（例如 info/success 保存 3 天，warning 保存 7 天，critical 保存 30 天，付费用户可自定义）。

7.1.5 手机 Push 推送
- App 注册设备 Push Token（FCM for Android，APNs for iOS）。
- 后端根据消息等级和项目默认策略判断是否推送（例如 critical 和 warning 推送，info/success 不推）。
- Push 通知仅携带 message_id 和简短摘要（标题），不包含完整敏感内容。
- 用户点击 Push 后打开 App 并跳转到消息详情页。
- 推送失败自动重试（最多 3 次）。

7.1.6 App 通知列表
- 展示全部通知，按时间倒序。
- 支持筛选：全部、未读、告警（warning+critical）、严重（critical）、今天、某项目、某来源。
- 列表项显示标题、摘要、来源、等级、时间、已读/未读状态。

7.1.7 通知详情页
- 显示标题、完整内容、来源、项目、等级、创建时间。
- 如有外部链接（url 字段），提供“查看详情”按钮跳转。
- 显示原始 JSON（可折叠）。
- 操作：标记已读/未读、删除、手动保留（pinned，不会被自动清理）。

7.1.8 已读/未读管理
- 新消息默认未读。
- 打开详情后自动标记已读（可配置）。
- 支持在列表页批量标记已读。

7.1.9 自动过期清理
- 后台定时任务（每小时运行）清理满足 expire_at < now() 且 pinned = false 的消息。
- 过期时间根据消息等级和项目/用户配置决定。
- 删除操作物理删除或软删除（标记 deleted，后续物理清理）。

7.1.10 基础设置
- 查看/管理已注册设备。
- 配置默认推送策略（哪些等级推送）。
- 查看账户信息和订阅状态。

7.2 后期增强功能（含自建版裁剪考虑）

7.2.1 规则引擎（高级）
- 支持按来源、项目、关键词、时间段等条件自定义推送与否。
- 支持重复消息聚合（同一项目+同一标题，短时间内只推送一次）。
- 支持“恢复通知”关联（如故障恢复时自动标记原告警已解决）。

自建版裁剪：此功能可能在自建版中限制规则数量或仅支持基础规则（如按等级），高级规则需要付费授权。

7.2.2 消息搜索
- 标题、内容关键词搜索。
- 多维度筛选（来源、项目、等级、时间范围）。
- 使用 SQLite FTS5 或简单 LIKE 查询（后期可迁移到 Elasticsearch 但非必须）。

自建版裁剪：搜索可能限制结果数量或仅支持最近 30 天数据搜索。

7.2.3 消息模板与集成向导
- 为常见来源（GitHub Actions, Prometheus, Grafana, Sentry, Cron 等）提供预置模板和 curl 示例。
- 降低接入门槛。

自建版裁剪：模板库可能仅包含基础模板，高级模板需付费。

7.2.4 国内推送通道适配
- 针对中国大陆 Android 用户，支持小米、华为、OPPO、vivo、荣耀等厂商推送通道。

自建版裁剪：自建版可能仅支持 FCM/APNs，厂商推送作为云托管专享或需额外配置。

7.2.5 自建（私有化部署）版本
- 提供 Docker 镜像和 Docker Compose 一键部署。
- 功能相对于云托管版有所裁剪，具体差异见第 15.4 节。
- 自建版可能采取源码开放（开源核心） + 商业插件模式，或免费使用 + 付费高级功能模式，具体策略待定。


---

8. 消息生命周期设计（保持不变，略）

8.1 默认保存策略
暂时无法在飞书文档外展示此内容
8.2 生命周期状态
消息状态包括：
- unread：未读
- read：已读
- ignored：已忽略
- pinned：已保留
- deleted：已删除
- expired：已过期
  8.3 过期清理逻辑
  系统定时任务清理满足以下条件的消息：
  expire_at < 当前时间
  且 pinned = false
  且 deleted = false
  8.4 为什么不默认永久保存
  大多数通知只在短时间内有价值。例如构建成功、备份完成、CPU 瞬时波动等消息，看过后长期价值很低。永久保存会导致数据库膨胀、列表噪声变大、产品变重。
  NoMail 的设计重点是：
  保证通知不会因为用户没及时看到而丢失，但不把低价值通知永久沉淀。


---

9. 页面设计

9.1 App 页面结构
登录/注册页
↓
通知列表页（默认）
↓
通知详情页
↓
项目列表页（管理 Webhook）
↓
项目详情/配置页
↓
设置页（账户、设备、订阅、自建版入口等）

（各页面内容基本沿用原设计，但项目/来源页需增加“创建项目”入口，设置页需包含“订阅管理”）

NoMail 是单用户应用，第一阶段不提供独立 Web 管理端。通知查看和少量管理功能都放在 App 内完成，包括来源/项目管理、Webhook Key 查看、默认推送策略和保存策略配置。
9.1 App 页面结构
登录页 / 初始化页
↓
通知列表页
↓
通知详情页
↓
来源/项目页
↓
项目配置页
↓
设置页
9.2 通知列表页
展示字段：
- 标题
- 摘要
- 来源
- 项目
- 等级
- 时间
- 已读/未读状态
  筛选项：
- 全部
- 未读
- 告警
- 严重
- 今天
- 某个项目
- 某个来源
  9.3 通知详情页
  展示字段：
- 标题
- 完整内容
- 来源
- 项目
- 消息等级
- 创建时间
- 外部链接
- 原始 JSON
- 推送状态
  操作：
- 标记已读
- 标记未读
- 删除
- 保留
- 复制内容
- 打开外部链接
  9.4 项目/来源页
  展示：
- 项目名称
- Webhook 地址
- Webhook Key
- 最近通知
- 启用/禁用
- 默认保存策略
- 默认推送策略
- curl 示例

---

10. 技术架构

10.1 总体架构（云托管 SaaS 版）
外部系统 → Webhook/API 接入层（负载均衡）→ Go 后端服务（Fiber）→ SQLite（或 PostgreSQL）→ 消息队列（可选）→ Push 分发服务 → FCM/APNs/厂商推送 → 手机 App
- 多租户设计：通过 project 和 user_id 隔离数据。
- 部署在云服务器（如 AWS、阿里云），使用 Nginx 反向代理 + Let's Encrypt 证书。
- 数据库初期使用 SQLite（单机），后期迁移到 PostgreSQL（支持更高并发和备份）。

10.2 后端推荐技术栈（云托管版）
- Go + Fiber（HTTP 框架）
- GORM（ORM）
- SQLite（开发/初期），PostgreSQL（生产）
- golang-migrate 管理 Schema
- robfig/cron 定时清理
- FCM SDK（Android）、APNs（iOS，后续）
- JWT 用于用户认证
- Docker + Docker Compose 部署

10.3 App 推荐技术栈
- 采用 Flutter，以同时支持 Android 和 iOS，降低开发成本。
- 使用 firebase_messaging 接收 Push。
- 状态管理使用 riverpod。
- 本地缓存使用 hive 或 sqflite。

10.4 自建版技术栈差异
- 自建版可复用同一套代码库，但通过编译开关或配置文件禁用部分付费功能。
- 自建版默认使用 SQLite，保持单进程、低依赖部署。
- 自建版需提供一键部署脚本和文档，并开放源代码（部分）。


---

11. 核心数据模型（云托管版）

新增 user 表，并调整其他表增加 user_id 外键，实现多租户隔离。

11.1 用户表 user
CREATE TABLE user (
id BIGINT PRIMARY KEY AUTO_INCREMENT,
email VARCHAR(255) NOT NULL UNIQUE,
password_hash VARCHAR(255) NOT NULL,
subscription_tier VARCHAR(30) DEFAULT 'free',  -- free, pro, enterprise
subscription_expire_at DATETIME,
created_at DATETIME NOT NULL,
updated_at DATETIME NOT NULL
);

11.2 项目表 notify_project
新增 user_id 字段，webhook_key 唯一。
CREATE TABLE notify_project (
id BIGINT PRIMARY KEY AUTO_INCREMENT,
user_id BIGINT NOT NULL,
name VARCHAR(100) NOT NULL,
webhook_key VARCHAR(128) NOT NULL UNIQUE,
enabled BOOLEAN DEFAULT TRUE,
default_level VARCHAR(30) DEFAULT 'normal',
retention_days INT DEFAULT 7,          -- 默认保存天数，可被等级覆盖
push_levels VARCHAR(100) DEFAULT 'warning,critical', -- 哪些等级推送
created_at DATETIME NOT NULL,
updated_at DATETIME NOT NULL,
FOREIGN KEY (user_id) REFERENCES user(id)
);

11.3 设备表 device
关联用户并长期保留设备元数据，登录状态由 session 表推导。
CREATE TABLE device (
id BIGINT PRIMARY KEY AUTO_INCREMENT,
user_id BIGINT NOT NULL,
device_id VARCHAR(128) NOT NULL,
name VARCHAR(128),
platform VARCHAR(32) NOT NULL,
device_type VARCHAR(32) NOT NULL,
manufacturer VARCHAR(128),
model VARCHAR(128),
os_version VARCHAR(64),
app_version VARCHAR(64),
app_build VARCHAR(64),
locale VARCHAR(32),
time_zone VARCHAR(64),
ip_address VARCHAR(64),
user_agent VARCHAR(255),
last_seen_at DATETIME,
created_at DATETIME NOT NULL,
updated_at DATETIME NOT NULL,
UNIQUE (user_id, device_id),
FOREIGN KEY (user_id) REFERENCES user(id)
);

11.4 Push Endpoint 表 push_endpoint
Push Token 与设备信息分离，并加密保存。
CREATE TABLE push_endpoint (
id BIGINT PRIMARY KEY AUTO_INCREMENT,
device_id BIGINT NOT NULL UNIQUE,
provider VARCHAR(32) NOT NULL,
environment VARCHAR(32) NOT NULL,
encrypted_token TEXT NOT NULL,
token_hash VARCHAR(64) NOT NULL UNIQUE,
enabled BOOLEAN DEFAULT TRUE,
registered_at DATETIME NOT NULL,
invalidated_at DATETIME,
last_error VARCHAR(500),
created_at DATETIME NOT NULL,
updated_at DATETIME NOT NULL,
FOREIGN KEY (device_id) REFERENCES device(id)
);

11.5 消息表 notify_message
关联项目即可，项目已关联用户。
CREATE TABLE notify_message (
id BIGINT PRIMARY KEY AUTO_INCREMENT,
project_id BIGINT NOT NULL,
source VARCHAR(100),
title VARCHAR(255) NOT NULL,
content TEXT,
level VARCHAR(30) DEFAULT 'normal',
status VARCHAR(30) DEFAULT 'unread',
url TEXT,
raw_payload JSON,
pinned BOOLEAN DEFAULT FALSE,
expire_at DATETIME,
read_at DATETIME,
created_at DATETIME NOT NULL,
updated_at DATETIME NOT NULL,
FOREIGN KEY (project_id) REFERENCES notify_project(id)
);

11.6 推送任务表 push_task（不变）


---

12. API 设计概要（变动不大，增加认证）

所有 API（除注册/登录外）需携带 JWT Token（Authorization: Bearer <token>）或 Session Cookie。

- 注册/登录：POST /api/auth/register, POST /api/auth/login
- Webhook 接收：POST /api/webhook/{webhook_key} （无需认证，通过 key 识别）
- 消息列表/详情：GET /api/messages，GET /api/messages/{id}（需认证，且只能查看自己项目的消息）
- 项目 CRUD：GET /api/projects，POST /api/projects，PUT /api/projects/{id}，DELETE /api/projects/{id}（需认证）
- 设备注册：POST /api/devices（需认证）
- 其他操作：标记已读、删除、保留等，均需认证并校验权限。

具体请求/响应格式与原有设计类似，但增加用户上下文。


---

13. 安全设计（保持不变，略）

13.1 Webhook Key
每个项目生成一个高强度随机 Webhook Key。外部系统通过该 Key 发送通知。
13.2 请求签名，可选
后续可支持 HMAC-SHA256 签名：
X-NoMail-Signature: sha256=xxxx
13.3 限流
对每个 Webhook Key 做限流，例如：
- 每分钟最多 60 条
- 每小时最多 1000 条
- 异常频率自动暂停
  13.4 敏感信息保护
- Push 通知中不放完整敏感内容
- Push 只携带 message_id 和简短摘要
- App 打开后再从后端拉取详情
- Bot Token、推送密钥等敏感配置不明文打印到日志


---

14. MVP 开发计划（调整）

- 阶段 0：用户体系与基础架构（认证、数据库迁移、项目模型）
- 阶段 1：后端核心（Webhook 接收、消息入库、列表/详情 API、基础限流）
- 阶段 2：移动端基础（登录、注册、列表、详情、项目管理）
- 阶段 3：Push 集成（FCM、APNs 接入，推送逻辑）
- 阶段 4：生命周期与规则（过期清理、等级推送策略）
- 阶段 5：订阅与付费（集成支付网关，如 Stripe 或 Lemon Squeezy，实现配额限制）
- 阶段 6：部署与公测（云服务器部署、监控、文档）
- 阶段 7（后续）：自建版发布（代码拆分、裁剪文档、Docker 镜像）


---

15. 商业化与订阅模式

15.1 定价模型（免费增值）
功能/配额
免费版
专业版（付费）
企业版/自建版（后期）
项目数量
3 个
无限
无限
每月消息量
500 条
10,000 条
自定义
消息保存天数
7 天
30 天（可自定义）
自定义
推送通道
FCM/APNs
FCM/APNs + 国内厂商
全部
自定义规则
仅按等级
完整规则引擎
完整规则引擎
重复消息聚合
无
支持
支持
模板库
基础
全部
全部
多设备
3 台
无限
无限
数据导出
无
支持 JSON 导出
支持
邮件/Webhook 备份推送
无
可选
可选
自建部署
不支持
不支持
提供（功能裁剪）

价格待定，可参考每月 $5~$10 美元。

15.2 免费版限制策略
- 当超出消息量限额时，仍接收消息但停止推送（或限制推送频率），并在 App 内提示升级。
- 超出保存天数的消息自动清理。
- 超过项目数量时无法新建项目。

15.3 付费方式
- 订阅制（月付/年付），通过主流支付平台（Stripe/Paddle/支付宝/微信支付）完成。
- 提供 14 天免费试用（专业版功能）。

15.4 自建版（后期）功能裁剪说明
为保障云托管版的商业价值，自建版将做以下裁剪（可配置打开，但默认关闭或需要商业授权）：
- 消息量限制：默认只保留最近 1000 条消息（可调，但建议付费解锁）。
- 高级规则引擎：仅支持按等级推送，不支持复杂条件或聚合。
- 国内厂商推送：需自行配置，默认不提供现成集成。
- 模板库：只提供基础模板（GitHub Actions、Cron），高级模板需手动编写。
- 用户界面：可能不带“订阅管理”页面。
- 更新机制：不提供在线升级，需手动拉取新镜像。
- 技术支持：社区支持，无官方 SLA。

自建版可能以 开源核心（AGPL） + 商业插件 或 源码可用但商业使用需付费 的模式发布，具体授权策略在后期细化。


---

16. 产品边界总结（调整）

NoMail 要做
- 云托管优先的通知收件箱
- Webhook/API 接收通知
- 手机 Push
- App 内集中查看与管理
- 短期历史记录与自动清理
- 免费增值商业模式
- 后期提供自建部署（功能适当裁剪）

NoMail 不做
- 完整 IM
- 群聊
- 多用户协作
- 复杂权限
- 工单系统
- 值班排班
- 企业级告警平台
- 长期知识库
- 复杂日志分析
- 独立 Web 管理端


---

17. 最终产品定义（修订）

NoMail 是一个以云托管 SaaS 优先、面向个人开发者、独立开发者和轻量运维用户的单用户通知收件箱。它通过 Webhook/API 聚合各类系统通知，推送到手机 App，并提供集中查看、筛选、项目管理和自动清理功能。同时，NoMail 采用免费增值模式，让用户免费使用基础功能，付费获得更高容量和高级特性。为满足自托管和隐私需求，后期将提供功能适当裁剪的自建版本，与云托管版形成差异化布局，确保商业可持续性。

核心价值不变：保证通知不会因为用户没及时看到而丢失，并在短期内可以集中回看；看过后没有价值的通知会自动清理。


---

18. 最简产品公式（不变）

（保留原公式）


---

修订日期：2026-06-18
