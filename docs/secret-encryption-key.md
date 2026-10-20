# Secret Encryption Key

`SECRET_ENCRYPTION_KEY` 用于加密和解密 API 存储的敏感信息，例如 webhook secret。

这个值必须是一个 base64 编码后的 32 字节随机密钥。项目会把它作为 AES-256-GCM 的密钥使用。

可以使用项目内置命令生成：

```bash
cargo run --bin gen-secret-key
```

输出示例：

```env
SECRET_ENCRYPTION_KEY=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx=
```

将生成的值写入 `.env`：

```env
SECRET_ENCRYPTION_KEY=xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx=
```

注意事项：

- 不要将真实密钥提交到 git。
- 部署后需要保持这个密钥稳定。如果密钥变更，之前已经加密的数据将无法解密。
- 不要手动填写密码或任意字符串。请使用生成命令，确保 base64 解码后正好是 32 字节随机数据。
- `.env.example` 中只应该放占位值，不应该放真实密钥。
