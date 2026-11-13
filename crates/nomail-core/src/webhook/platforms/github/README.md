# GitHub native webhook projections

## Official source and scope

The source is GitHub's MIT-licensed [`github/rest-api-description`](https://github.com/github/rest-api-description/tree/bd6df26d2a0104874fe5025a25ebcde356413364), commit `bd6df26d2a0104874fe5025a25ebcde356413364` (2026-11-12), specifically [`descriptions/api.github.com/api.github.com.json`](https://github.com/github/rest-api-description/blob/bd6df26d2a0104874fe5025a25ebcde356413364/descriptions/api.github.com/api.github.com.json). Its original notice is in `GITHUB-LICENSE.txt`.

Generation starts at **`x-webhooks`**, not every REST endpoint/resource. This snapshot contains **276 operation schemas across 76 canonical event names**, including Ping and actionless events. Referenced resource schemas are followed for their nested fields. These counts are not resource counts or a promise that every account/subscription receives all events. The document's `info.version` is not a webhook API version or a version header to send to GitHub. GitHub Enterprise Server/version-specific behavior is not exhaustively covered.

Official explanations: [Webhook events and payloads](https://docs.github.com/en/webhooks/webhook-events-and-payloads), [signature validation](https://docs.github.com/en/webhooks/using-webhooks/validating-webhook-deliveries), and [the announcement that webhook docs use OpenAPI](https://github.blog/changelog/2022-11-16-webhook-docs-generated-from-the-openapi-schema/).

## Coverage and compatibility

- Existing `GithubWebhookPayload`, Issue/PR, review/comment, repository/user, workflow/check, release/discussion, deployment and Git object wire paths remain available. `models.rs` retains maintained fields and compatibility variants such as string-or-object labels and integer-or-string repository timestamps. Two root fields now use explicit Rust unions: `comment` is a comment object or deployment-review text, and `workflow` is a workflow object or `workflow_dispatch` name. Match `GithubCommentValue` / `GithubWorkflowValue` before accessing object members; text is never wrapped in invented JSON `body`/`name` fields.
- `generated.rs` adds detailed nested fields from all pinned webhook operations: repository permissions/security/settings and links; PR auto-merge/links; installation/organization/enterprise context; security advisories, Dependabot/code/secret-scanning alerts and locations; Projects v2/items/status updates and typed old/new field values; rulesets/protection rules, merge groups, packages, sponsorship/marketplace, membership and other declared event data.
- Repeated objects and action-specific object alternatives are merged into tolerant native slots. `allOf` contributes fields from every branch; ambiguous object unions are not parsed by selecting the first all-optional struct. Actual mixed JSON kinds use untagged unions, without string/number/bool coercion. This is **not a strict per-event/action business validator**; it does not assert that every combination of optional properties is a valid GitHub event.
- Existing Rust structs expose added fields through a boxed `details` member to bound the stack footprint of the existing by-value model graph. It is `serde(flatten)`ed: **there is no new NoMail `details` wrapper in JSON**. Legitimate upstream properties named `details` (for example a secret-scanning location) keep their original paths. PR's existing flattened Issue fields also keep their native wire paths.
- Code constructing these Rust structs with exhaustive literals must account for the added `details` member, or use a struct update such as `..Default::default()`. New generated structs expose their declared fields directly.
- `GithubWebhook` remains `{ header, payload }`; nested fields keep paths such as `payload.pull_request.auto_merge`, `payload.installation.permissions`, `payload.alert.security_advisory` and `payload.projects_v2_item`. Application storage and read-time projection behavior are described in the [App README](../../../../../nomail-app/README.md#消息存储与读取投影).

## Precision and safe fallbacks

- Modeled properties preserve absent / explicit null / supplied values, including empty collections, false, zero and whitespace. Required upstream fields remain optional in read projections so one malformed field does not discard valid sibling fields or an authenticated delivery.
- Strings are never trimmed or coerced. Existing integer IDs/counts, GraphQL node-ID strings, repository timestamp variants and labels retain their wire types. Numeric fields use `GithubNumber` without an additional f64 conversion. Status/action labels remain open strings, including custom `repository_dispatch` actions.
- If an array element or map value cannot be extracted as its target type, the collection field is omitted **as a whole**, not filtered/reindexed. An invalid optional property inside a modeled object only omits that property, preserving its valid siblings. The complete original content remains in raw. This changes the old GitHub behavior that filtered malformed array elements.
- Upstream-defined arbitrary data (such as dispatch payloads/inputs or untyped schema branches) remains maps/JSON values rather than guessed resource types. Unknown fields, unsupported schema-version fields and malformed values remain in raw. A missing projected property is not proof that the original payload lacked it.
- Broader structural coverage does not fetch diffs, check annotations, workflow logs/artifacts, files, media, or other referenced resources. URL fields are references, not fetched details. It does not implement deployment approval/protection callbacks or other interactive workflows.
- JSON/form authentication, original-body HMAC-SHA256, header allowlisting/blacklisting, Delivery-ID deduplication, Ping control ACKs and ordinary ACK/priority behavior are unchanged. Built-in `to_message_content()` mappings remain unchanged.
- Business payloads are not automatically redacted. Security findings, custom payloads and other upstream data can contain sensitive values in raw or modeled fields; only synthetic/sanitized data is checked in as fixtures.

## Reproducibility and tests

The generator checks the immutable source SHA-256, reads the maintained declarations in `models.rs`, generates flattened additions and nested types, and runs rustfmt. Generation is an **explicit maintenance command**; Cargo builds/tests never download schemas. Python uses only the standard library, with no new runtime/build dependency.

Run from the repository root:

```bash
python3 crates/nomail-core/src/webhook/platforms/github/generate.py --fixtures
python3 crates/nomail-core/src/webhook/platforms/github/generate.py --fixtures --check
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s crates/nomail-core/src/webhook/platforms/github -p 'test_*.py'
```

For offline generation/checks, add `--schema /path/to/the/pinned/api.github.com.json`. Review upstream changes/license before changing the pin/hash; do not replace the hash to bypass a failure. Regenerate the application's OpenAPI before Rust tests/Clippy, as described in [CONTRIBUTING.md](../../../../../../CONTRIBUTING.md).

- `fixtures/official.json`: the six JSON examples attached to webhook operations in the pinned official document, with secret/token/contact/URL placeholders. These are examples, not a complete event corpus; some contain historical properties absent from the current schema, which remain in raw.
- `fixtures/operations.json`: 76 **synthetic, depth-bounded type probes**, merging source fields across actions for each event family. They are not captured deliveries or guaranteed upstream-valid full events. They test native field/type preservation, not every possible event/action combination or nested value.
- Additional targeted fixtures/tests cover nested PR/repository/installation data, security/Projects fields, dynamic input maps, type unions, presence states, malformed collections, raw reconstruction and signature/control behavior. Source header examples have known spelling inconsistencies (`discussions`, `project-v2`); the catalog uses canonical `x-github.subcategory` names instead. Runtime delivery headers are not rewritten by the generator.
