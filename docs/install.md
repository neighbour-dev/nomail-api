# 安装与部署

NoMail API 是后端服务，不附带 Web/App 客户端。部署后可通过 API 或兼容客户端管理账号、接收配置和消息，接口见 [OpenAPI](swagger.json)。

选择一种方式即可：

| 方式 | 环境要求 | 数据存储 |
|---|---|---|
| Docker Compose | Docker Engine、Compose v2 | 持久化卷中的 SQLite |
| 源码运行 | Rust 1.99+、系统编译工具 | 本地 SQLite 文件 |
| Cloudflare Workers | Rust 1.99+、Node.js 22+、Cloudflare 账号 | D1、Durable Objects |

以下 Server 命令从仓库根目录执行，Workers 命令会明确切换目录。请先获取源码；仓库地址替换为实际地址：

```bash
git clone <仓库地址> nomail-api
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

### 构建镜像

```bash
docker build -t nomail-api:local .
cp .env.example .env
```

编辑 `.env`，填写前面的密钥、用户名和密码；保留或调整：

```dotenv
JWT_ISSUER=nomail-api
PORT=3000
RUST_LOG=info
```

再编辑 `compose.yml`，将 `image` 改为刚构建的本地镜像：

```yaml
image: nomail-api:local
```

仓库中的默认镜像地址含 `your-github-username` 占位符，不能直接拉取。使用本地构建不需要 GHCR 凭据。

### 启动与检查

```bash
docker compose up -d --pull never
docker compose logs --tail=100 nomail-api
curl http://127.0.0.1:3000/api/health
```

健康检查正常时，响应的 `data.status` 和 `data.db` 均为 `ok`。若修改了 `PORT`，相应调整访问地址。Server 健康检查按来源 IP 限流，连续请求可能返回 429。

Compose 将数据库放在 `nomail-data` 卷的 `/data/nomail.db`，覆盖 `.env` 中的 `DATABASE_PATH`；容器根文件系统只读，数据库及 WAL/SHM 文件写入数据卷。不要使用 `docker compose down -v`，除非明确要删除全部数据。

修改 `.env` 后重新执行 `docker compose up -d` 以应用环境变量变化。首次登录改密后，再移除初始密码并重新执行该命令。

### 使用已有镜像

若你有可访问的镜像，无需执行前面的 `docker build`，但仍需创建并填写 `.env`。将 `compose.yml` 的 `image` 改为实际地址和版本，然后运行：

```bash
docker compose pull
docker compose up -d
```

如果是私有 GHCR 镜像，先用有权读取该 Package 的账号登录；Token 需具备相应读取权限，例如 classic PAT 的 `read:packages`：

```bash
docker login ghcr.io --username YOUR_GITHUB_USERNAME
```

在密码提示中输入 Token，不要将其写入 `.env`、Compose 或版本库。建议使用 Docker credential helper。仓库 CI 当前仅发布 `linux/amd64` 镜像；其他架构可自行构建。

## 3. 源码运行 Server

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
- 通过 `/api/webhook/configs` 创建接收配置，将响应中的相对 `url` 拼接到服务公开地址后，配置到上游平台。当前产品只开放 Generic、GitHub、飞书；协议库支持的其他平台不等于已开放接入。
- **飞书限制：**当前产品只有一个 `secret`，同时用作 Verification Token 和 Encrypt Key。两者不同的配置无法完整接入；普通事件不因签名缺失而回退为 Token 认证。
- 公网部署应使用 HTTPS，并在反向代理或 Cloudflare 设置请求限流与防滥用措施。Server 不直接信任转发头中的客户端 IP。
- 代理 SSE 时关闭响应缓冲，并设置大于 20 秒心跳间隔的读取超时。SSE 仅用于在线更新，无离线投递或重放，重连后应重新查询收件箱。

## 6. 备份、升级与账号恢复

### 备份与升级

- SQLite：停止服务后备份整个数据目录/卷，或使用 SQLite 在线备份工具。WAL 模式下不能在运行时只复制主数据库文件。
- D1：在更新前使用 Cloudflare 提供的导出或恢复能力保存备份，并确认恢复流程。
- 同时安全保存 `JWT_SECRET`、`JWT_ISSUER` 和 `SECRET_ENCRYPTION_KEY`。升级时保持它们稳定；更换加密密钥会导致旧配置无法解密。
- 当前建表脚本是新安装基线，不负责转换历史版本数据库。升级前核对目标版本说明并备份；程序不会自动清空已有开发数据。
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
