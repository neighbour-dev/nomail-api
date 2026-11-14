# nomail-app

开源版 NoMail 应用，供 Server 和 Workers 两种部署共同使用。平台协议来自 `nomail-core`；未来 SaaS 可以只依赖 Core，不依赖本 crate 的账号、收件箱或接口契约。

## 依赖方向

```text
Server / Workers → nomail-app → nomail-core
                    http → 业务模块
```

- `auth/`：账号、设备、会话、JWT、密码策略与原子存储接口。
- `message/`：已有消息的管理、查询和状态操作，以及 `MessageAccepted` 业务事件。`Message::from_received_event` 是唯一的完整消息构造入口，集中规范化和初始状态/保留规则，不访问数据库或运行时。
- `webhook/config/`：开源版接收配置、凭据加密、Key、启停及存储接口。
- `webhook/service.rs`：收件箱规范化规则；平台配置与认证构造在 Core，支持平台见根目录 [README](../../README.md)。
- `ingestion/`：完整接收用例：查配置、解密凭据、调用协议、处理控制请求、构造消息并幂等入箱。只依赖 `IngestionStore` 的配置查询和幂等插入；时间、ID 和密钥由运行时提供。控制请求不创建消息；只在首次持久化成功且返回 ID 一致时产生业务事件。
- `http/contract/`：集中定义唯一路由清单、所有 HTTP 请求/响应 DTO、响应包装和 SSE 事件，按认证、消息、配置、公共类型分文件。`http::*` 保留契约重导出。
- `http/`：完整接口执行、输入解析、业务结果转换、HTTP 错误映射、配置 URL、SSE 编码和 OpenAPI。两端一次接入 `http::execute`，不分别维护路径、方法、DTO 或逐接口 Handler。

业务不依赖 `http`。配置存储返回只含密文的内部 `ConfigRecord`，App 解密并校验平台与 provider 一致后生成无秘密、无 URL 的 `ConfigView`，HTTP 转换为 `ConfigResponse`；接收用例返回业务结果，`http::webhook::WebhookReply` 将其转换为 ACK/控制响应及可选 SSE 效果。连接与尽力而为的广播由平台实现，不增加可靠投递或重放。

业务和存储接口不使用 HTTP DTO。列表和游标使用轻量 `MessagePreview`，详情和状态更新返回 `MessageView`，原文查询返回 `RawContent`。HTTP 层负责命名、响应包装和业务类型转换。`tests/architecture.rs` 检查业务无 HTTP/Schema 依赖，HTTP Schema 仅定义在契约目录。

消息查询和批量操作、配置创建和修改在共享 HTTP 层完成一次输入转换，再调用用例：

- `http/contract/message.rs` 保留查询参数原始语法，转换为只读 `MessageQuery`；不会把枚举、布尔值和时间重新字符串化。Service 直接使用类型化过滤条件和已校验分页；配置分页同样先转为 `ConfigPage`。
- `http/contract/cursor.rs` 处理游标编码与查询转换；存储按接收时间、ID 执行范围查询，不统计总数。
- 批量输入先检查原始条数，再解析 ID 和操作、去重，转换为 UUID 列表及 `BatchAction`。
- `http/contract/config.rs` 定义配置请求外壳，内嵌 Core 的 `PlatformConfig`，在 HTTP 边界执行严格反序列化和 PATCH 转换；字段用法以 DTO 和生成的 OpenAPI 为准。
- `CreateConfig` / `UpdateConfig` 通过业务构造入口验证，不派生 Debug/Serialize。Service 生成 Key、加密平台配置并设置时间和默认值；存储只接收含密文的 `NewConfig` / `ConfigPatch`，原子更新 provider 与 encrypted_platform。空修改不写入。

## 消息存储与读取投影

消息保存通用展示字段、元数据、用户状态，以及三项 raw 数据，不保存类型化平台模型：

| 字段 | 内容 |
|---|---|
| `raw_headers` | Core 黑名单过滤后的头条目 JSON 数组 |
| `raw_payload` | 完整解码/解密后的 payload，不含 header/format/value 包装或密文 |
| `content_type` | payload 的序列化格式：JSON、文本或 URL 编码表单 |

JSON 保存为 JSON 文本，文本原样保存，已解码表单按 URL 编码序列化。raw 不是线级 HTTP 归档，过滤规则与敏感数据边界见 [Core](../nomail-core/README.md#事件模型与可读原始内容)。

- **接收：**复用接收阶段解析的 Core `PlatformData`，按平台规则生成并保存 `summary`，不再保存统一的 `source/content`。摘要合并空白，最多 30 个 Unicode 字符（按 Rust `char` 计数），超长时末尾使用 `…`（计入上限）。新接收消息建模失败、无法生成摘要或摘要为空时，由 App 保存固定摘要“无法解析消息内容”；不回退 raw JSON 或事件描述，不阻止已鉴权、已解码消息入库。
- **详情：**`MessageService` 从消息自身的 provider 和 raw 调用 Core `parse_raw()`，组成临时 `MessageView`。详情响应不再提供统一的 `source/content`，通过 `platformData` 提供结构化内容，完整载荷由独立 raw 接口提供。不重新验签、解密、读取当前凭据或重算摘要等通用字段和用户状态；飞书 Encrypt Key 轮换不影响已保存的解密数据。
- **模型升级：**Core 平台模型更新后，已保存 raw 消息的读取投影也可能出现新增字段或字段集合变化。仅更新平台模型不需要为此改写 raw 存储或重新入库，也不重算已保存的通用展示字段及用户状态；需要完整解码内容时仍读取原文视图。
- **投影失败：**`platform_data` 为 `None`，`platform_data_error` 提供诊断，不阻止详情和状态操作；投影失败不能把已提交的状态更新变成错误响应。
- **原文：**独立查询返回 `RawContent` 存储视图，不恢复平台模型、不解析 payload 或校验 MIME。HTTP 层只解码头条目数组，payload 保持数据库原字符串，contentType 不用于设置 HTTP 响应头。
- **列表与状态：**分页和游标列表直接读取已存储的 `summary`，不加载 raw 或解析平台模型；旧记录的空摘要原样返回，不在查询时补写错误文案。搜索暂时匹配 `title + summary`。状态修改只更新指定字段，不重写摘要或 raw。

两端的消息摘要迁移直接新增默认空字符串的 `summary` 列，并删除 `source/content` 列，不判断旧库、不回填旧消息摘要、不清空现有消息；平台原生载荷及 raw 中同名字段保持不变。

存储接口表达原子操作，不是通用 CRUD。刷新单次消费、改密撤销、所有权检查、局部更新、幂等约束分别由 SQLite 事务和 D1 条件语句/batch 实现；不能拆成无保护的多步读写。

`MessageStore` 仅负责已有消息的读取与状态变更，`IngestionStore` 负责接收所需的查询与创建。两者可以由同一个 SQLite/D1 适配对象实现，共用数据库，不重复入库 SQL。消息管理测试通过工厂构造初始数据，接收测试只实现两个存储操作，无需为消息管理方法提供占位实现。

## HTTP 契约与检查

`src/http/contract/routes.rs` 是路径、方法、鉴权、输入输出类型和 OpenAPI 的唯一清单；`src/http/dispatch.rs` 绑定 Operation 与用例。`http/contract/events.rs` 定义 SSE 事件，`http/sse.rs` 负责帧编码和心跳。新增接口在 App 定义并绑定，不要求两端补写 Handler；遗漏分发会编译失败。

HTTP 契约以路由、DTO 和生成的 OpenAPI 为准，数据库契约以迁移及存储实现为准；不兼容变更和升级要求见 [CHANGELOG.md](../../CHANGELOG.md)。

平台实现两个小边界：

- `HttpRequest` 提供请求头部及延迟 Body 读取；共享执行器先匹配路由、再鉴权、再读取和解析 Body。适配器逐块检查 4 MiB 限制，共享层额外检查最终长度。Webhook 原字节和重复头不被 JSON 转换破坏。
- `Environment` 分别组装 Auth/Message/Config/Ingestion 存储和运行时，返回 `Services`，不按接口定义方法。消息管理与接收的存储类型独立，不要求消息存储实现接收接口或 `Clone`。服务配置只在已知非健康检查路由上读取，健康检查无需账号或加密配置。原子操作仍由原存储接口负责。

每个平台在 `src/runtime.rs` 中实现 `Runtime`，满足 App 的认证、配置和接收 trait，`services.rs` 负责组装。时间、随机数和密码执行由运行时提供，App 测试使用可控替身。

`HttpOutcome` 包含已编码响应或 SSE 订阅指令，以及首次持久化后的实时效果。两端只写响应、建立连接和执行尽力而为的广播，不解析 DTO 或匹配 Operation。

错误在所属层定义，HTTP 层统一映射状态码和公开消息；运行时只适配框架响应和传输错误。启动及账号维护工具不借用 HTTP 错误，请求内部错误仅在最终响应边界记录一次安全诊断。唯一约束冲突由存储层识别，不根据任意平台错误猜测 HTTP 409。

路径参数在共享分发边界转为 `ConfigId` 或 `Uuid`，用例不重复解析。请求先鉴权，再读取 Body，最后按调用参数顺序验证 ID 和 JSON；路由参数只进行一次 URL 解码。

```sh
cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json
cargo test --locked -p nomail-app --all-targets
cargo test --locked -p nomail-app --features openapi --lib
cargo check --locked -p nomail-app --target wasm32-unknown-unknown
```

相关测试前先重新生成 OpenAPI，不复用可能过期的文件。`openapi` feature 只用于文档/测试，普通构建不链接 Schema 生成器；完整检查与生成文件规则见[贡献指南](../../CONTRIBUTING.md#tests-and-checks)。
