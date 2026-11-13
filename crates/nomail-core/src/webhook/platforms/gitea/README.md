# Gitea native webhook models

## Source and external event scope

The baseline is official Gitea **v28.3.0**, pinned to commit [`4f5a3e13e5f3aca85d830e3a9024c893b0bbf1e1`](https://github.com/go-gitea/gitea/tree/4f5a3e13e5f3aca85d830e3a9024c893b0bbf1e1). `source-lock.json` records SHA-256 hashes of the reviewed public sources. The upstream MIT notice is retained in `GITEA-LICENSE.txt`; upstream Go source files are not vendored or built into the Rust application.

The external contract is derived from three connected entry points, not every API struct or every type whose name ends in `Payload`:

- [`modules/webhook/type.go`](https://github.com/go-gitea/gitea/blob/4f5a3e13e5f3aca85d830e3a9024c893b0bbf1e1/modules/webhook/type.go): `AllEvents()` declares **26 external event types**, and `Event()` gives their normalized `X-Gitea-Event` names.
- [`services/webhook/payloader.go`](https://github.com/go-gitea/gitea/blob/4f5a3e13e5f3aca85d830e3a9024c893b0bbf1e1/services/webhook/payloader.go): the outbound dispatch switch maps those events to **14 native payload types**. `hook.go` and their referenced `modules/structs` types define the body fields.
- [`user.go`](https://github.com/go-gitea/gitea/blob/4f5a3e13e5f3aca85d830e3a9024c893b0bbf1e1/modules/structs/user.go) and [`modules/json/jsonv2.go`](https://github.com/go-gitea/gitea/blob/4f5a3e13e5f3aca85d830e3a9024c893b0bbf1e1/modules/json/jsonv2.go): custom serialization and JSON-v2 options also affect actual wire data. Declared struct tags alone are not sufficient.

`WorkflowDispatchPayload` and `WorkflowCallPayload` are internal Actions data in this snapshot, not extra external subscriptions supported by this model. The source snapshot and event counts are not a claim of compatibility with every historical or future Gitea version, nor with Forgejo/Gogs/GitHub payloads.

## Native fields and Rust types

- `GiteaWebhook { header, payload }` keeps Gitea's JSON hierarchy. Exact event type, normalized event and Delivery ID remain in the allowlisted header model, not invented body fields.
- User IDs remain signed `i64`, including negative system-user IDs. Workflow definition IDs remain strings; workflow run/job IDs remain integers. Text, labels, timestamps and state/action strings are not trimmed, coerced, or replaced by display summaries.
- `User.MarshalJSON()` emits `username` in addition to `login`. `GiteaUser` preserves both supplied keys independently. It does not manufacture `username` when only `login` is received or force differing supplied values to match.
- Repository payloads use `*User` at `organization`, while package/workflow payloads use `*Organization`. `GiteaOrganization` retains the union of the native fields of both shapes, including `type`, login/source metadata, flags and counters from User. This does not introduce a JSON discriminator or copy GitHub types.
- Go slices with pointer members (`[]*User`, `[]*Label`, `[]*Attachment`, workflow steps, and other declared pointer slices) use **`Vec<Option<T>>`** inside their presence-aware field. `[null, {...}]` remains two elements in the same order. Missing collection, explicit null collection, empty collection and null elements are distinct states.
- Existing scalar fields still use `JsonField<T>` for missing / explicit null / supplied values. Model structs gain fields, so exhaustive Rust struct literals must be updated or use `..Default::default()`. Rust consumers of pointer slices must handle `Option<T>` elements explicitly.

These are tolerant read projections, not strict upstream request DTOs. Fields required by the sender may be absent in a sparse stored payload. Unknown/version-specific fields remain in raw. If an element or map value cannot be read as its declared type, the entire collection field is omitted rather than filtering/reindexing entries. An invalid optional property in an otherwise valid object only omits that property, preserving valid sibling properties.

JSON/form verification is unchanged: Gitea signs the original JSON text, or the original JSON text obtained by form decoding, **not** the encoded form body and not reserialized JSON. Only the native `X-Gitea-Signature` authentication path is accepted. Event/Delivery metadata, replay handling, ordinary ACKs and basic message mappings remain unchanged. The model does not fetch attachments, repository data, logs, or other references.

## Contract audit and fixtures

`source_contract.py` checks the maintained Rust field declarations against the committed native contract. The full maintenance path checks all pinned source hashes, uses the Go standard-library AST parser to follow external payload references, and reviews custom serializers. It rejects unsupported source types or newly encountered custom serializers rather than guessing their representation. Recursive repository references are audited by native type mapping, not expanded indefinitely.

Artifacts:

- `fixtures/source-contract.json`: 61 reachable Go types (including scalar aliases and payload types), the 26-event outbound map, original JSON tags/types and `User.MarshalJSON`'s extra `username` field. The checker verifies 49 Go/Rust type mappings, including the two organization shapes and minimal workflow PR references; these numbers are not event counts.
- `fixtures/wire-probes.json`: **28 synthetic probes**, zero-valued and populated examples for each of the 14 payload types. They are serialized from the reviewed original type declarations and custom User method using Gitea's JSON-v2 nil-slice/map options. Synthetic nil pointer elements, negative IDs, whitespace and zero/null states are intentional. They are not captured deliveries or guaranteed valid business-event instances; Go type correctness does not validate subscription/action/permission semantics.

Cargo builds/tests read the committed artifacts offline and never download Gitea or invoke Go. The declaration checker uses only Python's standard library:

```bash
python3 crates/nomail-core/src/webhook/platforms/gitea/source_contract.py --offline --check
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s crates/nomail-core/src/webhook/platforms/gitea -p 'test_*.py'
```

Full regeneration/reproducibility checks are explicit maintenance commands requiring an existing **Go 1.26+** toolchain with the `jsonv2` experiment. The script uses standard-library packages only and disables toolchain/module downloads:

```bash
python3 crates/nomail-core/src/webhook/platforms/gitea/source_contract.py
python3 crates/nomail-core/src/webhook/platforms/gitea/source_contract.py --check
```

Add `--source-dir /path/to/pinned/source` to avoid source downloads; every listed file must match its locked hash. Upgrades require reviewing source/serializer/codec changes before updating the lock and artifacts. Regenerate application OpenAPI before compiling Rust tests/Clippy, following [CONTRIBUTING.md](../../../../../../CONTRIBUTING.md).

Rust tests compare all supplied probe fields and scalar types/values, preserve pointer-slice null positions, round-trip models, restore raw projections, and exercise all 26 event/header pairs in both probe modes. Server and Workers integration tests share these probes for signed JSON/form delivery, storage/read projection, deduplication and independent state updates. Tests do not exhaust every possible value or Gitea version.
