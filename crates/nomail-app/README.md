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
- `webhook/service.rs`：开源版支持的平台和收件箱规范化规则。
- `ingestion/`：唯一完整接收用例：查配置、解密凭据、调用协议、区分控制请求、构造消息并幂等入箱。只依赖 `IngestionStore` 的有效配置查询和幂等插入两个操作，不持有 `MessageService` 或继承 `MessageStore`。时间、ID 和密钥由运行时提供；控制请求不创建消息 ID，不入库。只在首次持久化成功且返回 ID 一致时产生业务事件。
- `http/contract/`：集中定义唯一路由清单、所有 HTTP 请求/响应 DTO、响应包装和 SSE 事件，按认证、消息、配置、公共类型分文件。`http::*` 保留契约重导出。
- `http/`：完整接口执行、输入解析、业务结果转换、HTTP 错误映射、配置 URL、SSE 编码和 OpenAPI。两端一次接入 `http::execute`，不分别维护路径、方法、DTO 或逐接口 Handler。

业务不依赖 `http`。配置用例和存储返回无 URL 的 `ConfigView`，HTTP 转换为 `ConfigResponse`；接收用例返回业务结果，`http::webhook::WebhookReply` 将其转换为 ACK/控制响应及可选 SSE 效果。连接与尽力而为的广播由平台实现，不增加可靠投递或重放。

业务和存储接口不使用 HTTP DTO。认证使用 `Login` / `UpdateAccount`、`AccountInfo` / `SessionTokens` / `DeviceInfo`；消息返回轻量 `MessagePreview` 或 `MessageWithConfig`，数量直接返回 `u64`。HTTP 转换负责 camelCase、空字符串回退、嵌套展开和包装；业务 `pagination::Page` 与契约 `Page` 通过一个泛型转换连接。`Claims` 是 JWT 内容，实体的 Serde 可以用于存储，不属于 HTTP 契约。`tests/architecture.rs` 检查业务无 HTTP/Schema 依赖，应用 wire schema 仅定义在契约目录。

消息查询和批量操作、配置创建和修改在共享 HTTP 层完成一次输入转换，再调用用例：

- `http/contract/message.rs` 保留查询参数原始语法，转换为只读 `MessageQuery`；不会把枚举、布尔值和时间重新字符串化。Service 直接使用类型化过滤条件和已校验分页；配置分页同样先转为 `ConfigPage`。
- 批量输入转换为 UUID 列表及 `BatchAction`。先检查原始 1～100 条限制，再解析 ID 和操作、去重；Service 不再匹配任意 action 字符串。
- `http/contract/config.rs` 分别定义创建和修改请求。创建的 name/provider 由 Serde 要求提供；修改的缺省和空字符串仍表示保留，null 和错误类型仍拒绝。通用 JSON 解析器不再二次解析或维护端点必填字段特判。
- `CreateConfig` / `UpdateConfig` 通过业务构造入口验证，字段不对外开放构造，不派生 Debug/Serialize。Service 仍负责生成 Key、加密凭据、时间和默认值，存储只接收含密文的 `NewConfig` / `ConfigPatch`。全空修改仍是只读，不更新接收配置的时间。

这些类型不是通用命令框架；权限、资源状态和原子写入检查仍保留在用例与存储边界。

存储接口表达原子操作，不是通用 CRUD。刷新单次消费、改密撤销、所有权检查、局部更新、幂等约束分别由 SQLite 事务和 D1 条件语句/batch 实现；不能拆成无保护的多步读写。

`MessageStore` 仅负责已有消息的读取与状态变更，`IngestionStore` 负责接收所需的查询与创建。两者可以由同一个 SQLite/D1 适配对象实现，共用数据库，不重复入库 SQL。消息管理测试通过工厂构造初始数据，接收测试只实现两个存储操作，无需为消息管理方法提供占位实现。

## HTTP 契约与检查

`src/http/contract/routes.rs` 是路径、方法、鉴权、输入输出类型和 OpenAPI 的唯一清单；`src/http/dispatch.rs` 是唯一的 Operation → 用例绑定及请求/结果转换边界。消息列表存储仅选预览列；D1 用私有存储行解码，不借用 HTTP 字段别名。`http/contract/events.rs` 定义事件，`http/sse.rs` 只负责帧编码和心跳。新增操作只需修改 App；若遗漏共享分发会编译失败，不要求两个部署补写 Handler。公开 HTTP/数据库行为保持不变。

平台实现两个小边界：

- `HttpRequest` 提供请求头部及延迟 Body 读取；共享执行器先匹配路由、再鉴权、再读取和解析 Body。适配器逐块检查 4 MiB 限制，共享层额外检查最终长度。Webhook 原字节和重复头不被 JSON 转换破坏。
- `Environment` 分别组装 Auth/Message/Config/Ingestion 存储和运行时，返回 `Services`，不按接口定义方法。消息管理与接收的存储类型独立，不要求消息存储实现接收接口或 `Clone`。服务配置只在已知非健康检查路由上读取，健康检查无需账号或加密配置。原子操作仍由原存储接口负责。

每个平台仅在自己的 `src/runtime.rs` 中实现一个具体 `Runtime`，直接满足 App 的认证、配置和接收三个窄 trait；`services.rs` 只负责组装，不再转发到 `auth/runtime.rs`。时间和随机数直接使用平台能力，随机数错误分别映射到所属用例，不经认证错误中转。Server 密码运算仍使用 `spawn_blocking`，本地账号维护也复用同一个 Runtime；App 测试继续使用自己的可控替身。

`HttpOutcome` 包含已编码的普通响应或 SSE 订阅指令，以及首次持久化后的实时效果。两端只写响应、建立连接和执行尽力而为的广播，不再出现 Operation 匹配、Endpoint 引用或请求 DTO 解析。`HttpError` 将安全诊断保留到平台最终日志边界，不泄露内部错误文本。

Server 的 `ApiError` 仅包装共享 `HttpError` 并实现 Actix `ResponseError`，不再直接转换业务或数据库错误；状态码和公开消息策略仍由 App HTTP 层统一定义。配置加载返回 `ConfigLoadError`，数据库初始化返回分阶段的 `ConnectError`，账号维护和令牌/密码操作返回认证错误，纯加密工具保留共享工具的错误。启动与工具调用不借用 HTTP 错误；请求内部错误仅在最终响应边界记录一次安全诊断。

Workers 的配置和随机数同样不依赖 HTTP 错误：配置失败仅在 `services.rs` 映射一次，随机数/UUID 返回平台工具错误，最终 `ApiError` 只适配共享 HTTP 错误和平台传输失败。唯一约束冲突由 D1 存储层识别，不根据任意平台错误猜测 409。Server 已移除旧 `crypto` 和令牌包装入口，直接使用 App 的安全函数、认证服务与平台 Runtime。

路径参数在共享 HTTP 分发边界按需转为 `ConfigId`（正数业务 ID）或 `Uuid`，用例不再接收配置 ID 字符串，也不重复解析消息 UUID。保留先鉴权、再读取 Body、再按调用参数顺序验证 ID 和 JSON 的行为；路由解析阶段仍仅进行原有的一次 URL 解码。

```sh
cargo test --locked -p nomail-app --all-targets
cargo test --locked -p nomail-app --features openapi --lib
cargo check --locked -p nomail-app --target wasm32-unknown-unknown
cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json
```

OpenAPI 是生成文件，不手工维护。`openapi` feature 只用于文档/测试，普通构建不链接 Schema 生成器。
