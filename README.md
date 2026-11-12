# NoMail API

English | [简体中文](README.zh-CN.md)

A single-user, self-hosted notification inbox API: receive webhooks and store notifications from different sources as manageable, traceable messages.

## Features

- Receive webhooks from Generic, GitHub, Gitea, App Store Connect, GitLab, Stripe, Slack, Feishu, Telegram, and Shopify, with platform-specific strongly typed configuration, source authentication, platform verification requests, and idempotent storage.
- Preserve structured platform data and readable original content, with message search, read status, pinning, soft deletion, and batch operations.
- Manage accounts, devices, and sessions, with SSE for live inbox updates.
- Two deployment options share the same business logic and HTTP API:
  - **Server**: Rust, Actix Web, SeaORM, and SQLite; run with Docker or from source.
  - **Cloudflare Workers**: official worker-rs, D1, and Durable Objects.

This repository contains the NoMail backend, which works with the NoMail App to provide a notification inbox. The backend currently does not offer public registration or push notifications such as APNs/FCM. SSE does not support replay or offline delivery; message expiration does not yet trigger automatic cleanup.

## Documentation

- [Project documentation](https://git.neighbour.sg/neighbour/nomail-docs)
- [Installation and deployment](https://git.neighbour.sg/neighbour/nomail-docs/src/branch/main/guide/install.md)
- [Changelog](CHANGELOG.md)
- [Git tag conventions](docs/tagging.md)
- [Contributing](CONTRIBUTING.md)
- HTTP API definition (OpenAPI 3.1): newer [GitHub Releases](https://github.com/neighbour-dev/nomail-api/releases) include a `swagger.json` asset. Choose the asset matching your running version. For older releases without this asset, [generate it](CONTRIBUTING.md#updating-api-documentation) from the corresponding version's source code.
- [Platform protocol library](crates/nomail-core/README.md)
- [Open-source edition application architecture](crates/nomail-app/README.md)

## License

Unless explicitly stated otherwise for specific files or third-party components, the code and documentation in this repository are licensed under the **Functional Source License 1.1 with Apache-2.0 as the future license (`FSL-1.1-ALv2`)**, with **NEIGHBOUR PTE. LTD.** as the licensor. See [LICENSE](LICENSE).

FSL is a source-available license, not an OSI-approved open-source license. It permits uses such as internal use but restricts competitive commercial uses as defined in its terms. Each version becomes available under the Apache License 2.0 two years after the licensor makes it available; the entire project does not convert on a single shared date.

Versions previously released under AGPL-3.0-only retain their existing licenses. Third-party components retain their respective licenses.
