# nomail-app

开源版 NoMail 应用，供 Server 和 Workers 两种部署共同使用。平台协议来自 `nomail-core`；未来 SaaS 可以只依赖 Core，不依赖本 crate 的账号、收件箱或接口契约。

## 依赖方向

```text
Server / Workers → nomail-app → nomail-core
                    http → 业务模块
```

- `auth/`：账号、设备、会话、JWT、密码策略与原子存储接口。
- `message/`：消息管理、查询、状态、保留规则与 `MessageAccepted` 业务事件。
- `webhook/config/`：开源版接收配置、凭据加密、Key、启停及存储接口。
- `webhook/service.rs`：开源版支持的平台和收件箱规范化规则。
- `ingestion/`：唯一完整接收用例：查配置、解密凭据、调用协议、区分控制请求、幂等入箱。时间、ID 和密钥由运行时提供；控制请求不创建消息 ID，不入库。只在首次持久化成功后产生业务事件。
- `http/`：唯一接口清单、输入解析、响应包装、HTTP 错误映射、配置 URL、SSE 编码和 OpenAPI。两端一次接入该清单，不分别维护路径、方法或 DTO。

业务不依赖 `http`。配置用例和存储返回无 URL 的 `ConfigView`，HTTP 转换为 `ConfigResponse`；接收用例返回业务结果，`http::webhook::WebhookReply` 将其转换为 ACK/控制响应及可选 SSE 效果。连接与尽力而为的广播由平台实现，不增加可靠投递或重放。

业务输入输出可以直接使用 Serde；不为无差异的数据机械复制 DTO。HTTP 特有的 URL、响应包装、状态码和帧格式不能进入业务或存储接口。`tests/architecture.rs` 检查该边界。

存储接口表达原子操作，不是通用 CRUD。刷新单次消费、改密撤销、所有权检查、局部更新、幂等约束分别由 SQLite 事务和 D1 条件语句/batch 实现；不能拆成无保护的多步读写。

## HTTP 契约与检查

`src/http/routes.rs` 是路径、方法、鉴权、输入输出类型和 OpenAPI 的唯一清单。平台统一入口调用共享解析器并穷尽绑定 Operation；新增操作漏绑定会编译失败。公开 HTTP/数据库行为不因 crate 拆分改变。

```sh
cargo test --locked -p nomail-app --all-targets
cargo test --locked -p nomail-app --features openapi --lib
cargo check --locked -p nomail-app --target wasm32-unknown-unknown
cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json
```

OpenAPI 是生成文件，不手工维护。`openapi` feature 只用于文档/测试，普通构建不链接 Schema 生成器。
