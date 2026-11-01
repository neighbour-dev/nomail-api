# 更新日志

本文件记录项目面向用户的重要变更，版本遵循语义化版本规范。
项目版本统一定义在根目录 `Cargo.toml` 的 `[workspace.package].version`，四个 crate 继承同一个版本，不独立发布版本。

## [Unreleased]

### 新增

- GitHub Actions 在普通分支 push 和 PR 时，使用原生 AMD64、ARM64 runner 执行格式检查和 Rust 测试。
- 推送 `v*` 版本标签后，原生构建双架构 Docker 镜像并发布到 GHCR，合并为同一个多架构镜像标签。
- 镜像发布成功后自动创建 GitHub Release，生成更新说明；支持预发布版本。

### 变更

- 四个 Rust crate 统一继承 workspace 版本，当前版本为 `0.1.0`。
- 发布前校验 Git 标签与 workspace 版本一致，避免镜像、Release 与源码版本不匹配。
- 补充多架构镜像部署、版本管理和标签发布说明。
- 拆分检查与发布工作流：普通提交只检查，版本标签触发发布并复用检查流程。

<!-- 发布时将 Unreleased 中的条目移入对应版本，例如：## [0.1.0] - YYYY-MM-DD；保留新的 Unreleased 区域。 -->
