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
- `http/`：唯一接口清单、完整接口执行、输入解析、响应包装、HTTP 错误映射、配置 URL、SSE 编码和 OpenAPI。两端一次接入 `http::execute`，不分别维护路径、方法、DTO 或逐接口 Handler。

业务不依赖 `http`。配置用例和存储返回无 URL 的 `ConfigView`，HTTP 转换为 `ConfigResponse`；接收用例返回业务结果，`http::webhook::WebhookReply` 将其转换为 ACK/控制响应及可选 SSE 效果。连接与尽力而为的广播由平台实现，不增加可靠投递或重放。

业务输入输出可以直接使用 Serde；不为无差异的数据机械复制 DTO。HTTP 特有的 URL、响应包装、状态码和帧格式不能进入业务或存储接口。`tests/architecture.rs` 检查该边界。

存储接口表达原子操作，不是通用 CRUD。刷新单次消费、改密撤销、所有权检查、局部更新、幂等约束分别由 SQLite 事务和 D1 条件语句/batch 实现；不能拆成无保护的多步读写。

`MessageStore` 仅负责已有消息的读取与状态变更，`IngestionStore` 负责接收所需的查询与创建。两者可以由同一个 SQLite/D1 适配对象实现，共用数据库，不重复入库 SQL。消息管理测试通过工厂构造初始数据，接收测试只实现两个存储操作，无需为消息管理方法提供占位实现。

## HTTP 契约与检查

`src/http/routes.rs` 是路径、方法、鉴权、输入输出类型和 OpenAPI 的唯一清单；`src/http/dispatch.rs` 是唯一的 Operation → 用例绑定。新增操作只需修改 App；若遗漏共享分发会编译失败，不要求两个部署补写 Handler。公开 HTTP/数据库行为保持不变。

平台实现两个小边界：

- `HttpRequest` 提供请求头部及延迟 Body 读取；共享执行器先匹配路由、再鉴权、再读取和解析 Body。适配器逐块检查 4 MiB 限制，共享层额外检查最终长度。Webhook 原字节和重复头不被 JSON 转换破坏。
- `Environment` 分别组装 Auth/Message/Config/Ingestion 存储和运行时，返回 `Services`，不按接口定义方法。消息管理与接收的存储类型独立，不要求消息存储实现接收接口或 `Clone`。服务配置只在已知非健康检查路由上读取，健康检查无需账号或加密配置。原子操作仍由原存储接口负责。

`HttpOutcome` 包含已编码的普通响应或 SSE 订阅指令，以及首次持久化后的实时效果。两端只写响应、建立连接和执行尽力而为的广播，不再出现 Operation 匹配、Endpoint 引用或请求 DTO 解析。`HttpError` 将安全诊断保留到平台最终日志边界，不泄露内部错误文本。

```sh
cargo test --locked -p nomail-app --all-targets
cargo test --locked -p nomail-app --features openapi --lib
cargo check --locked -p nomail-app --target wasm32-unknown-unknown
cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json
```

OpenAPI 是生成文件，不手工维护。`openapi` feature 只用于文档/测试，普通构建不链接 Schema 生成器。
