# 安装与部署

NoMail API 是后端服务，不附带 Web/App 客户端。部署后可通过 API 或兼容客户端管理账号、接收配置和消息，接口见 [OpenAPI](swagger.json)。

选择一种方式即可：

| 方式 | 环境要求 | 数据存储 |
|---|---|---|
| Docker Compose | Docker Engine、Compose v2 | 持久化卷中的 SQLite |
| 发布的二进制 | Linux amd64 或 arm64，glibc 2.39+（如 Ubuntu 24.04） | 本地 SQLite 文件 |
| 源码运行 | Rust 1.99+、系统编译工具 | 本地 SQLite 文件 |
| Cloudflare Workers | Rust 1.99+、Node.js 22+、Cloudflare 账号 | D1、Durable Objects |

以下 Server 命令从仓库根目录执行，Workers 命令会明确切换目录。请先获取源码：

```bash
git clone https://github.com/neighbour-dev/nomail-api.git
cd nomail-api
```

## 1. 准备密钥与初始账号

**不要直接使用示例文件中的密码或密钥。**

下文复制配置文件的命令仅用于首次安装。已有 `.env` 或 `.dev.vars` 时不要覆盖；升级前先备份配置，不要重新生成已有部署的加密密钥。

| 配置 | 用途与要求 |
|---|---|
| `JWT_SECRET` | Token 签名密钥；使用独立、高强度随机值 |
| `JWT_ISSUER` | Token 签发者，默认 `nomail-api` |
| `SECRET_ENCRYPTION_KEY` | 接收配置凭据的加密密钥；Base64 编码的 32 字节随机数据 |
| `INITIAL_USERNAME` | 数据库账号表为空时创建的初始用户名 |
| `INITIAL_PASSWORD` | 初始密码，至少 8 个字符，包含英文字母和数字，最多 72 字节 |

如果安装了 OpenSSL，可分别运行下面两条命令，第一条输出填入 `JWT_SECRET`，第二条填入 `SECRET_ENCRYPTION_KEY`：

```bash
openssl rand -hex 32
openssl rand -base64 32
```

也可以使用项目的密钥生成工具，见[加密密钥说明](secret-encryption-key.md)。JWT 密钥和凭据加密密钥应独立生成。

Server 在首次启动时初始化账号；Workers 在首次登录时初始化。**先使用初始账号登录并修改密码，再移除 `INITIAL_PASSWORD`**。已有账号不会因重新设置初始配置而被覆盖。

## 2. Docker Compose

### 准备配置

默认使用 GHCR 发布的镜像，无需本地构建。`compose.yml` 已配置：

```yaml
image: ghcr.io/neighbour-dev/nomail-api:latest
```

也可单独拉取镜像（不会启动服务）：

```bash
docker pull ghcr.io/neighbour-dev/nomail-api:latest
```

首次安装时创建配置文件：

```bash
cp .env.example .env
```

编辑 `.env`，填写前面的密钥、用户名和密码；保留或调整：

```dotenv
JWT_ISSUER=nomail-api
PORT=3000
RUST_LOG=info
```

### 启动与检查

```bash
docker compose pull
docker compose up -d
docker compose logs --tail=100 nomail-api
curl http://127.0.0.1:3000/api/health
```

健康检查正常时，响应的 `data.status` 和 `data.db` 均为 `ok`。若修改了 `PORT`，相应调整访问地址。健康检查不设置应用级请求限流；有需要时可在反向代理或 CDN 配置入口限流。

Compose 将数据库放在 `nomail-data` 卷的 `/data/nomail.db`，覆盖 `.env` 中的 `DATABASE_PATH`；容器根文件系统只读，数据库及 WAL/SHM 文件写入数据卷。不要使用 `docker compose down -v`，除非明确要删除全部数据。

修改 `.env` 后重新执行 `docker compose up -d` 以应用环境变量变化。首次登录改密后，再移除初始密码并重新执行该命令。

### 可选：本地构建

如需部署本地修改，可自行构建镜像：

```bash
docker build -t nomail-api:local .
```

将 `compose.yml` 的 `image` 改为：

```yaml
image: nomail-api:local
```

同样需要创建并填写 `.env`，然后启动（不要执行 `docker compose pull`）：

```bash
docker compose up -d --pull never
```

### 镜像版本

发布镜像支持 `linux/amd64` 和 `linux/arm64`，Docker 会自动选择当前机器对应的架构。`latest` 跟随稳定版发布更新，不包含预发布版本。生产部署建议将 `compose.yml` 中的 `latest` 替换为明确的稳定版本标签（如 `vX.Y.Z`）；版本变更见[更新日志](../CHANGELOG.md)。

## 3. 二进制或源码运行 Server

### 使用发布的二进制

从 [GitHub Releases](https://github.com/neighbour-dev/nomail-api/releases) 下载对应架构的 `nomail-api-<版本>-linux-amd64.tar.gz` 或 `nomail-api-<版本>-linux-arm64.tar.gz`，以及 `SHA256SUMS`，放在同一目录。二进制在 Ubuntu 24.04 上使用 Cargo 原生构建，无需安装 Rust；运行环境要求 glibc 2.39 或更新版本，不适用于默认使用 musl 的 Alpine。当前功能无需额外安装 CA 证书或时区数据。

以下以 amd64 为例，将文件名中的 `vX.Y.Z` 替换为下载的版本：

```bash
sha256sum --ignore-missing -c SHA256SUMS
mkdir -p nomail-server
tar -xzf nomail-api-vX.Y.Z-linux-amd64.tar.gz -C nomail-server
cd nomail-server
cp .env.example .env
```

填写 `.env` 中的密钥和初始账号后启动：

```bash
./nomail-api
```

压缩包还包含 `gen-secret-key`、`reset-account` 和许可证。可执行 `./gen-secret-key` 生成凭据加密密钥，使用 `./reset-account` 代替下文源码部署的 `./target/release/reset-account`。数据库默认位于 `./data/nomail.db`；生产环境应通过服务管理器配置工作目录、环境变量及自动重启。

### 从源码构建

安装 Rust 1.99+ 及平台编译工具；无需另外部署数据库服务。

```bash
cp .env.example .env
cargo run --locked -p nomail-server --bin gen-secret-key
```

生成器输出完整的 `SECRET_ENCRYPTION_KEY=...` 配置行。填好 `.env` 中其余密钥和初始账号后，构建并启动：

```bash
cargo build --locked --release -p nomail-server --bins
./target/release/nomail-api
```

默认监听 `0.0.0.0:3000`，数据库为 `./data/nomail.db`，目录、数据库和表会自动创建。`.env` 从进程当前目录加载；生产环境应通过服务管理器配置工作目录、环境变量及自动重启。

Server 使用 SQLite WAL 和外键约束，不需要 PostgreSQL，也不提供旧 PostgreSQL 数据迁移。

## 4. Cloudflare Workers

Workers 使用 D1 和 Durable Objects。bcrypt 和常驻 SSE 有 CPU、连接及费用开销，部署前确认实际套餐限制，不承诺免费套餐适用。

### 安装构建工具

```bash
rustup target add wasm32-unknown-unknown
cargo install worker-build --locked --version 0.8.7
cd crates/nomail-workers
npm ci
```

确保 `worker-build` 在 `PATH` 中。所有后续 Wrangler 命令均在 `crates/nomail-workers` 目录执行。

### 创建资源并部署

```bash
npx wrangler login
npx wrangler d1 create nomail
```

将创建结果中的 `database_id` 写入 `wrangler.toml`。文件中的全零 ID 只用于本地开发；若调整数据库名称，也同步修改 `database_name`。按需修改 Worker 的 `name`，保留代码使用的绑定名 `DB` 和 `REALTIME`。

依次配置远程 Secret，在提示中输入实际值：

```bash
npx wrangler secret put JWT_SECRET
npx wrangler secret put SECRET_ENCRYPTION_KEY
npx wrangler secret put INITIAL_USERNAME
npx wrangler secret put INITIAL_PASSWORD
```

`JWT_ISSUER` 位于 `wrangler.toml` 的 `[vars]` 中。然后确认目标数据库，初始化并部署：

```bash
npx wrangler d1 migrations apply DB --remote
npm run deploy
```

Wrangler 会调用 `worker-build --release` 并部署 `Realtime` Durable Object。使用部署输出的地址检查 `/api/health`，首次登录并修改密码后删除远程初始密码：

```bash
npx wrangler secret delete INITIAL_PASSWORD
```

本地 `.env` 或 `.dev.vars` 不会代替远程 Secret 配置。

### 本地试运行

不连接远程数据库：

```bash
cp .dev.vars.example .dev.vars
```

先填写 `.dev.vars` 的密钥和账号，再运行：

```bash
npx wrangler d1 migrations apply DB --local
npm run dev
```

使用 Wrangler 输出的本地地址。`.dev.vars` 和本地数据库不得提交到仓库；本地修改不会更新远程部署。

## 5. 部署后的配置

- 通过 `PATCH /api/auth/account` 修改账号；请求需要 Bearer Token，并提供 `currentPassword` 及 `username`、`newPassword` 中至少一项。改密会撤销全部登录会话。
- 通过 `/api/webhook/configs` 创建接收配置，将响应中的相对 `url` 拼接到服务公开地址后，配置到上游平台。支持 Generic、GitHub、GitLab、Stripe、Slack、飞书、Telegram 和 Shopify；创建 NoMail 配置不会自动创建平台侧订阅。
- 平台配置使用严格的 `platform.provider` + `platform.credentials`，不再接受旧顶层 `provider`、`secret` 或配置 `source`。平台详细设置见 [接收协议与配置协议参考](webhook-platform-reference-2026-10-30.md)。
- 公网部署应使用 HTTPS，并在反向代理或 Cloudflare 设置请求限流与防滥用措施。
- 通过反向代理部署时，应正确设置并清洗客户端 IP 转发头；设备 IP 仅用于展示，不应作为访问控制依据。
- 代理 SSE 时关闭响应缓冲，并设置大于 20 秒心跳间隔的读取超时。SSE 仅用于在线更新，无离线投递或重放，重连后应重新查询收件箱。

### 创建和修改 Webhook 配置

配置管理接口要求 `Authorization: Bearer <accessToken>`。例如创建飞书加密接收配置：

```http
POST /api/webhook/configs
Content-Type: application/json
Authorization: Bearer <accessToken>
```

```json
{
  "name": "Feishu events",
  "description": "Work notifications",
  "platform": {
    "provider": "feishu",
    "credentials": {
      "verificationToken": "example-verification-token",
      "encryptKey": "example-encryption-key"
    }
  }
}
```

`name` 必填，1–100 字符；`description` 可省略，最多 500 字符；创建时默认启用。平台及认证模式使用下表的精确大小写，不接受未知字段或位置数组。通用秘密字段要求非空白且最多 500 字符，这是接收端的限制，不是上游平台统一规定。

| provider | credentials 示例 | 约束 |
|---|---|---|
| `generic` | `{"type":"none"}` 或 `{"type":"hmacSha256","secret":"example-secret"}` | 仅 Generic 允许显式无认证；none 不接受 secret |
| `github` | `{"secret":"example-secret"}` | 必须配置与 GitHub Webhook 一致的 Secret |
| `gitlab` | `{"type":"signingToken","signingToken":"whsec_ZXhhbXBsZS1rZXk="}` 或 `{"type":"secretToken","secretToken":"example-token"}` | 两种模式互斥；Signing Token 去前缀后须为非空 Base64 密钥，不能因签名缺失回退旧 Token |
| `stripe` | `{"signingSecrets":["whsec_example"]}` | 接收端允许 1–5 个密钥用于轮换；使用端点/环境对应的密钥原文，不采用 GitLab 的解码方式；轮换完成后主动移除旧密钥 |
| `slack` | `{"signingSecret":"example-secret"}` | 不是 Bot Token、Client Secret 或旧 Verification Token |
| `feishu` | `{"verificationToken":"example-token","encryptKey":"example-key"}` | 两个独立值；省略 encryptKey 或设为 null 表示仅 Token 模式，非空 Encrypt Key 启用加密事件模式 |
| `telegram` | `{"secretToken":"example_token"}` | 1–256 字符，只允许英文字母、数字、下划线和短横线；不是 Bot Token |
| `shopify` | `{"clientSecret":"example-secret"}` | 应用 Client Secret，不是店铺 Admin API Access Token |

平台配置整体加密存储，响应只返回安全摘要，例如 `platform: {"provider":"feishu","authentication":"feishuEncrypted"}`，不会回显 credentials 或秘密值。响应还包含 id、name、key、enabled、description 和相对 url。请在安全的外部凭据管理中保存原值，不要依赖查询接口找回。

`PATCH /api/webhook/configs/{id}` 中省略字段表示保留；`{}` 不写库、不改变更新时间。传入 `platform` 时必须提供完整的新平台配置并整体替换，不能只传 provider 或单个秘密字段。顶层字段不接受 null；description 可以用 `""` 清除，name 不允许空字符串。切换平台会影响后续接收，不会重写已有消息的平台快照，接收 key 保持不变；切换 Telegram Bot 应创建新配置以隔离去重作用域。

启停仍用 `PATCH /api/webhook/configs/{id}/enabled` 和 `{"enabled":false}`；删除仍为软删除。平台事件自带的消息 source 保留，缺少时用平台名兜底，不再从接收配置取默认来源。

## 6. 备份、升级与账号恢复

### 备份与升级

- SQLite：停止服务后备份整个数据目录/卷，或使用 SQLite 在线备份工具。WAL 模式下不能在运行时只复制主数据库文件。
- D1：在更新前使用 Cloudflare 提供的导出或恢复能力保存备份，并确认恢复流程。
- 同时安全保存 `JWT_SECRET`、`JWT_ISSUER` 和 `SECRET_ENCRYPTION_KEY`。升级时保持它们稳定；更换加密密钥会导致旧配置无法解密。
- 当前建表脚本是新安装基线，不负责转换历史版本数据库。升级前核对目标版本说明并备份；程序不会自动清空已有开发数据。
- **强类型 Webhook 配置是不兼容更新：**新基线移除了 webhook_configs.source/encrypted_secret，改为 encrypted_platform，不提供旧数据库或旧配置迁移。仅重新提交旧请求不能修复旧 Schema。应在备份后使用新的 SQLite 路径/数据卷或新的 D1 数据库，按对应部署步骤初始化并重新创建接收配置；旧账号、消息等不会自动导入。Server 会拒绝缺少新配置列的旧 Schema，Workers 同样不能复用旧 D1 基线。不要为升级直接删除原数据库或生产卷。
- Server 与 Workers 的数据库编码不同，不支持直接交换 SQLite/D1 数据库文件。
- Docker 升级时构建/拉取目标版本，再重新执行 `docker compose up -d`；源码部署则重新构建并重启服务。不要为升级删除数据卷。

### Server 忘记密码

使用与运行服务相同的数据库路径及配置执行：

```bash
./target/release/reset-account --username admin --password '<新密码>'
```

Docker 部署可执行：

```bash
docker compose exec nomail-api reset-account --username admin --password '<新密码>'
```

`--username` 可省略以保留原用户名；重置会撤销全部会话。密码经命令行参数传递，应在可信终端操作，注意命令历史和进程参数的可见性。

Workers 不提供上述本地账号重置工具，也没有公开的找回密码接口，请妥善保存账号和备份。
