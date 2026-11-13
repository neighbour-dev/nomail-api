# Stripe native projections

## Sources and reproducibility

- [Official OpenAPI](https://github.com/stripe/openapi/tree/a045100cf4af0acb548cb9c6f236bb117f83458f/latest): public GA API **2026-10-28.endive**, commit `a045100cf4af0acb548cb9c6f236bb117f83458f` (2026-11-12).
- [Snapshot envelope](https://docs.stripe.com/api/events/object) and [event catalog](https://docs.stripe.com/api/events/types).
- [Event formats](https://docs.stripe.com/events/how-events-work) and [Events v2](https://docs.stripe.com/api/v2/core/events/object): the initial thin notification is not a fetched full event/resource.
- `fixtures/resources.json` contains 16 synthetic official resource examples from the same commit's [`openapi/fixtures3.json`](https://github.com/stripe/openapi/blob/a045100cf4af0acb548cb9c6f236bb117f83458f/openapi/fixtures3.json). Example contact and client-secret values are replaced by non-production placeholders. These fixtures also contain legacy fields absent from the public GA schema: those are asserted to remain in raw, not claimed as modeled.
- The specification and fixtures are MIT licensed; the original notice is in `STRIPE-LICENSE.txt`. No new runtime/build dependencies are added.

`generate.py` verifies the source API version and SHA-256, generates `generated.rs`, and runs rustfmt. It never runs during Cargo builds or tests. Explicit maintenance commands (Python 3 standard library and the existing Rust toolchain only):

```bash
python3 crates/nomail-core/src/webhook/platforms/stripe/generate.py --fixtures
python3 crates/nomail-core/src/webhook/platforms/stripe/generate.py --fixtures --check
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s crates/nomail-core/src/webhook/platforms/stripe -p 'test_*.py'
```

Use `--schema /path/to/openapi.spec3.json --fixture-source /path/to/fixtures3.json` with `--fixtures` for offline generation/checks. Both inputs must match the pinned hashes; presence of an existing file is not a trust check. To upgrade, review the upstream API changes and license, explicitly update the commit/version/hashes, regenerate, and run Rust tests plus fresh application OpenAPI checks. Do not replace a hash just to bypass a failure.

## Coverage and wire format

- `StripeWebhookPayload::Snapshot` models `object = "event"`, integer Unix `created`, `api_version`, account/context, request metadata, `data.object`, and `data.previous_attributes`.
- `StripeWebhookPayload::Thin` models `object = "v2.core.event"`, string `created`, context, reason/request, related object and `snapshot_event`. Event types remain open strings: both `v1.*` and `v2.*` resources can generate thin notifications. Extra received `data`/`changes` hashes remain open maps, never interpreted as a resource snapshot or fabricated by an API query.
- `StripeResource` has **200 live/deleted resource branches**, generated from the pinned public specification's named component schemas that declare a native `object` enum, and **2,249 generated definitions** for resources, referenced nested objects, inline structures, unions and literal object tags. This is a resource count, **not a count of webhook event types**. Payment, Checkout, Billing, Connect, Issuing, Treasury, and v2 resource schemas are included; a schema's existence does not mean that every resource is delivered by a subscribed event. Referenced API/expansion structures are modeled as well, even when they are not directly delivered as webhook resource snapshots.
- Known resources dispatch using `data.object.object`, not ID prefixes or guessed event-name prefixes. This does not validate that the event type and resource form an allowed business combination. Unknown resources, missing discriminators, and unknown expandable objects use an object-map fallback without inventing a tag. Unknown top-level envelopes produce no platform model; accepted deliveries still retain raw and get the existing generic display fallback.
- Resource/expandable enums serialize **untagged**. There are no `Snapshot`, `Thin`, `PaymentIntent`, `Unknown`, or other NoMail discriminator wrappers in JSON. All paths stay native, e.g. `event.payload.data.object.customer` and `event.payload.related_object.type`.
- Expandable fields accept their actual string ID or typed expanded resource, including distinct deleted-resource branches. Undiscriminated server/client option schemas are merged by native field name rather than guessed by first-match optional structs.

## Precision boundaries

These are tolerant **read projections**, not upstream request validators or a complete cross-version Stripe SDK:

- Every modeled property preserves missing / explicit null / actual value, including empty maps/arrays/strings, false and zero. Required fields are intentionally optional in projections: one malformed field must not erase valid siblings or reject an authenticated delivery.
- Strings are not trimmed/coerced. Minor-unit amounts and Unix timestamps are integers; decimal strings remain strings; numeric fields use `StripeNumber` without an additional f64 conversion. Status/type enums remain strings to retain future upstream values.
- A malformed typed collection is omitted as a whole, not filtered/reindexed. The complete collection stays in raw. Open hashes are modeled as maps, typed where the specification provides a value type.
- `previous_attributes` remains an open sparse old-value map: it is not safe to deserialize it as a complete resource, lose null metadata deletions, or impose current resource types on historical values.
- Unknown fields in known resources, invalid field types, resource schemas absent from the public specification, and older/newer version-specific fields remain in raw. The reference version is not a claim that every historical/future `api_version` is compatible. Public OpenAPI does not enumerate every event-to-resource association or event-specific thin data shape.
- No API keys, outbound HTTP, resource retrieval, media downloads, financial decisions, or summary/priority changes are introduced. In particular, modeling an Issuing authorization resource **does not implement the synchronous approval/decline callback**; do not use an inbox endpoint as an authorization decision service.

Rust consumers now match the envelope and resource enums before accessing typed fields. HTTP JSON retains its existing native nesting, but a known resource's projected fields are now schema-based rather than an unrestricted JSON copy; clients needing the complete payload must use raw.

## UI adaptation and actual deliveries

- Model coverage does not determine message volume. Actual deliveries depend on the subscribed event types, the Stripe products/account scope in use, and business activity. One business operation can generate several events, while each event carries its own payload rather than all modeled resources at once.
- Handle `platformData: null` and `platformDataError` before accessing the platform model, using the stored common display/raw as a fallback. Otherwise distinguish Snapshot/Thin using `event.payload.object`. For a Snapshot, inspect `event.payload.data.object.object` when present to choose a resource-specific view, with a generic fallback for missing/unknown resource kinds. Event `type` and resource `object` describe different things; neither the 200 resource branches nor the 2,249 definitions is an event-subscription list.
- Render only supplied fields. Distinguish a missing property, explicit null, zero/false/empty values, and an expandable string ID versus an object. A modeled property does not promise that Stripe will include it; fields absent from the projection may also be unsupported or malformed in the original payload, so use raw when that distinction matters.
- Thin notifications do not automatically provide payment, invoice, or account details. Treat `related_object` as a reference, not a fetched resource; its API URL is not automatically a user-facing link. The inbox does not retrieve missing details.
- Detailed `platformData` enables client-specific cards, but this change does **not** implement those cards or richer built-in summaries. The existing common mapping still uses an event title and raw content, with an explicit not-fetched note for Thin.
- Tests exercise discriminator dispatch/round trips for all 200 resource branches, 16 sanitized official resource examples, and targeted nested/presence/expansion/malformed-field cases. They do not exhaustively verify every generated nested field, every event/resource association, or every API version.
