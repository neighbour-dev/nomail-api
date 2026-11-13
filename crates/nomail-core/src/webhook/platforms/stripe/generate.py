#!/usr/bin/env python3
"""Generate tolerant native Stripe resource projections; no third-party packages.

Only this explicit maintenance command downloads anything. Rust builds/tests are
offline. The pinned specification is MIT licensed; see STRIPE-LICENSE.txt.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import urllib.request

COMMIT = "a045100cf4af0acb548cb9c6f236bb117f83458f"
VERSION = "2026-10-28.endive"
SHA256 = "27e7f9681bfe4423708023338fd14096600581718fc2e15efd0d946d6a522c25"
URL = f"https://raw.githubusercontent.com/stripe/openapi/{COMMIT}/latest/openapi.spec3.json"
FIXTURES_URL = f"https://raw.githubusercontent.com/stripe/openapi/{COMMIT}/openapi/fixtures3.json"
FIXTURES_SHA256 = "c94c6c106a8743e559609792d2591c9b975e5588b4ea70425ce9377c1c6d0026"
FIXTURE_RESOURCES = (
    "payment_intent", "charge", "checkout.session", "customer", "subscription",
    "invoice", "invoiceitem", "refund", "dispute", "payment_method", "product",
    "price", "payout", "transfer", "credit_note", "account",
)
KEYWORDS = set("as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true try type unsafe use where while yield abstract become box do final macro override priv typeof unsized virtual gen".split())


def pascal(name):
    return "".join(word[:1].upper() + word[1:] for word in re.split(r"[^a-zA-Z0-9]+", name) if word)


def field_name(name):
    return "r#" + name if name in KEYWORDS else name


def fixture_subset(source):
    """Keep synthetic official examples, replacing example contact/secret values."""
    if hashlib.sha256(source).hexdigest() != FIXTURES_SHA256:
        raise ValueError("Fixture source SHA-256 mismatch")
    resources = json.loads(source)["resources"]

    def sanitize(value, key=""):
        if isinstance(value, dict):
            return {k: sanitize(v, k) for k, v in value.items()}
        if isinstance(value, list):
            return [sanitize(v, key) for v in value]
        if isinstance(value, str):
            if "secret" in key:
                return "example_not_a_credential"
            if "email" in key and "@" in value:
                return "customer@example.invalid"
            if key in ("phone", "support_phone"):
                return "+12025550123"
        return value

    return json.dumps({key: sanitize(resources[key]) for key in FIXTURE_RESOURCES}, indent=2, sort_keys=True) + "\n"


class Generator:
    def __init__(self, spec):
        self.schemas = spec["components"]["schemas"]
        self.definitions = {}
        self.names = {"StripeResource": "<resource union>", "StripeNumber": "<numeric primitive>"}
        self.unions = {}
        self.roots = sorted(
            key for key, schema in self.schemas.items()
            if schema.get("properties", {}).get("object", {}).get("enum")
        )

    def reserve(self, name, identity):
        if name in self.names and self.names[name] != identity:
            raise ValueError(f"Rust name collision: {name}: {identity} / {self.names[name]}")
        self.names[name] = identity
        return name

    def reference(self, reference):
        prefix = "#/components/schemas/"
        if not reference.startswith(prefix):
            raise ValueError(f"Unsupported reference: {reference}")
        key = reference[len(prefix):]
        name = self.reserve("Stripe" + pascal(key), key)
        if name not in self.definitions:
            self.definitions[name] = ""  # Break recursive references.
            self.definitions[name] = self.model(name, self.schemas[key])
        return f"Box<{name}>"

    def rust_type(self, schema, name):
        if "$ref" in schema:
            return self.reference(schema["$ref"])
        if "anyOf" in schema or "oneOf" in schema:
            return self.union(schema.get("anyOf", schema.get("oneOf")), name)
        kind = schema.get("type")
        if kind == "string":
            return "String"  # Open enums retain future Stripe values verbatim.
        if kind == "integer":
            return "i64"
        if kind == "number":
            return "StripeNumber"
        if kind == "boolean":
            return "bool"
        if kind == "array":
            item = schema["items"]
            ty = self.rust_type(item, name + "Item")
            if item.get("nullable"):
                ty = f"Option<{ty}>"
            return f"Vec<{ty}>"
        if kind == "object":
            if schema.get("properties"):
                self.reserve(name, json.dumps(schema, sort_keys=True))
                if name not in self.definitions:
                    self.definitions[name] = ""
                    self.definitions[name] = self.model(name, schema)
                return f"Box<{name}>"
            additional = schema.get("additionalProperties", True)
            ty = self.rust_type(additional, name + "Value") if isinstance(additional, dict) and additional else "Value"
            if isinstance(additional, dict) and additional.get("nullable"):
                ty = f"Option<{ty}>"
            return f"BTreeMap<String, {ty}>"
        # Fail closed in the generator rather than silently weakening a model.
        raise ValueError(f"Unsupported schema at {name}: {schema}")

    def object_tag(self, values):
        if not all(isinstance(value, str) for value in values):
            raise ValueError("Non-string object discriminator")
        name = "Stripe" + "Or".join(pascal(value) for value in values) + "ObjectTag"
        self.reserve(name, "<object tag:" + json.dumps(values) + ">")
        if name not in self.definitions:
            branches = ", ".join(f"{pascal(value)} = {json.dumps(value)}" for value in values)
            self.definitions[name] = f"stripe_tag!({name} {{ {branches} }});"
        return name

    def union(self, branches, name):
        if len(branches) == 1:
            return self.rust_type(branches[0], name)
        name += "Value"
        identity = json.dumps(branches, sort_keys=True)
        if identity in self.unions:
            return f"Box<{self.unions[identity]}>"
        self.reserve(name, identity)
        self.unions[identity] = name
        self.definitions[name] = ""
        objects = [self.schemas[b["$ref"].split("/")[-1]] for b in branches if "$ref" in b]
        # Stripe's server/client option unions often have no discriminator and
        # overlapping shapes. Merge their fields instead of picking the first
        # all-optional struct and silently losing the other branch's fields.
        if len(objects) == len(branches) and any(
            not obj.get("properties", {}).get("object", {}).get("enum") for obj in objects
        ):
            properties = {}
            for obj in objects:
                for key, prop in obj.get("properties", {}).items():
                    if key in properties:
                        old = properties[key]
                        # Descriptions, nullability, and enum values don't alter
                        # our tolerant wire types. Other differences form a union.
                        def shape(s):
                            return {k: v for k, v in s.items() if k not in ("description", "nullable", "enum", "maxLength", "title") and not k.startswith("x-")}
                        if shape(old) != shape(prop):
                            prop = {"anyOf": old.get("anyOf", [old]) + prop.get("anyOf", [prop])}
                    properties[key] = prop
            self.definitions[name] = self.model(name, {"type": "object", "properties": properties})
            return f"Box<{name}>"
        variants = []
        for branch in branches:
            ty = self.rust_type(branch, name + "Value")
            variant = pascal(branch["$ref"].split("/")[-1]) if "$ref" in branch else pascal(branch["type"])
            if "$ref" in branch:
                # Never guess between resource variants with absent tags.
                obj = self.schemas[branch["$ref"].split("/")[-1]]
                tags = obj.get("properties", {}).get("object", {}).get("enum")
                condition = " | ".join(json.dumps(t) for t in tags) if tags else None
                deleted = obj.get("properties", {}).get("deleted", {}).get("enum") == [True]
                if condition:
                    comparison = "==" if deleted else "!="
                    condition = f'matches!(value.get("object").and_then(Value::as_str), Some({condition})) && value.get("deleted").and_then(Value::as_bool) {comparison} Some(true)'
                elif len(objects) == 1:
                    condition = "value.is_object()"
                else:
                    raise ValueError(f"Untagged union mixing resource and non-resource: {name}")
            else:
                condition = {"string": "value.is_string()", "integer": "value.is_i64()", "number": "value.is_number()", "boolean": "value.is_boolean()"}[branch["type"]]
            variants.append((variant, ty, condition))
        self.definitions[name] = self.enum(name, variants)
        return f"Box<{name}>"

    def enum(self, name, variants, resources=False):
        lines = [f"stripe_union!({name} {{"]
        for variant, ty, condition in variants:
            lines.append(f"    {variant}({ty}) if |value: &Value| {condition},")
        lines.append("});")
        if resources:
            # A test table covers every supported live/deleted discriminator.
            lines += ["", "#[cfg(test)]", "pub(super) const RESOURCE_TAGS: &[(&str, bool)] = &["]
            for key in self.roots:
                schema = self.schemas[key]
                for tag in schema["properties"]["object"]["enum"]:
                    deleted = schema["properties"].get("deleted", {}).get("enum") == [True]
                    lines.append(f"    ({json.dumps(tag)}, {str(deleted).lower()}),")
            lines.append("];")
        return "\n".join(lines)

    def model(self, name, schema):
        if schema.get("type") != "object":
            raise ValueError(f"Expected structured object: {name}")
        properties = schema.get("properties", {})
        tags = properties.get("object", {}).get("enum")
        guard = ""
        if tags:
            alternatives = " | ".join(json.dumps(tag) for tag in tags)
            guard = f' [|value: &Value| value.get("object").is_none_or(|tag| tag.is_null() || matches!(tag.as_str(), Some({alternatives}))) ]'
        lines = [f"stripe_model!({name}{guard} {{"]
        for key, prop in sorted(properties.items()):
            if not re.fullmatch(r"[a-z_][a-z0-9_]*", key):
                raise ValueError(f"Unsupported field identifier: {key}")
            ty = self.object_tag(prop["enum"]) if key == "object" and prop.get("enum") else self.rust_type(prop, name + pascal(key))
            lines.append(f"    {field_name(key)}: {ty} = {json.dumps(key)},")
        lines.append("});")
        return "\n".join(lines)

    def generate(self):
        variants = []
        seen = set()
        for key in self.roots:
            ty = self.reference("#/components/schemas/" + key)
            schema = self.schemas[key]
            tags = schema["properties"]["object"]["enum"]
            deleted = schema["properties"].get("deleted", {}).get("enum") == [True]
            for tag in tags:
                if (tag, deleted) in seen:
                    raise ValueError(f"Ambiguous resource tag: {tag}, deleted={deleted}")
                seen.add((tag, deleted))
            condition = " | ".join(json.dumps(t) for t in tags)
            comparison = "==" if deleted else "!="
            condition = f'matches!(value.get("object").and_then(Value::as_str), Some({condition})) && value.get("deleted").and_then(Value::as_bool) {comparison} Some(true)'
            variants.append((pascal(key), ty, condition))
        header = f"""//! Generated by generate.py; DO NOT EDIT.
//! Stripe API {VERSION}, stripe/openapi commit {COMMIT}.
//! Source SHA-256: {SHA256}
//! Derived from MIT-licensed Stripe specifications; see STRIPE-LICENSE.txt.
//! All fields are tolerant presence-aware projections, not API request validators.

use super::support::*;
use serde_json::Value;
use std::collections::BTreeMap;

pub const STRIPE_MODEL_API_VERSION: &str = {json.dumps(VERSION)};
pub const STRIPE_MODEL_SPEC_COMMIT: &str = {json.dumps(COMMIT)};
"""
        parts = [header, self.enum("StripeResource", variants, resources=True)]
        parts += [text for _, text in sorted(self.definitions.items())]
        return "\n\n".join(parts) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema", type=Path, help="Use an already downloaded pinned specification")
    parser.add_argument("--check", action="store_true", help="Check that generated.rs is up to date")
    parser.add_argument("--fixtures", action="store_true", help="Also regenerate/check the sanitized official fixture subset")
    parser.add_argument("--fixture-source", type=Path, help="Use already downloaded pinned fixture source")
    args = parser.parse_args()
    source = args.schema.read_bytes() if args.schema else urllib.request.urlopen(URL, timeout=60).read()
    if hashlib.sha256(source).hexdigest() != SHA256:
        raise ValueError("Specification SHA-256 mismatch; do not generate from an unreviewed version")
    spec = json.loads(source)
    if spec["info"]["version"] != VERSION:
        raise ValueError("Specification API version mismatch")
    generator = Generator(spec)
    output = generator.generate()
    # Rustfmt through stdin keeps --check read-only and output deterministic.
    output = subprocess.run(["rustfmt", "--edition", "2024"], input=output, text=True, capture_output=True, check=True).stdout
    path = Path(__file__).with_name("generated.rs")
    if args.check:
        if path.read_text() != output:
            raise SystemExit("Stripe generated.rs is stale")
    else:
        path.write_text(output)
    if args.fixtures:
        source = args.fixture_source.read_bytes() if args.fixture_source else urllib.request.urlopen(FIXTURES_URL, timeout=60).read()
        fixtures = fixture_subset(source)
        path = Path(__file__).with_name("fixtures") / "resources.json"
        if args.check:
            if path.read_text() != fixtures:
                raise SystemExit("Stripe resource fixtures are stale")
        else:
            path.parent.mkdir(exist_ok=True)
            path.write_text(fixtures)
    print(f"Stripe {VERSION}: {len(generator.roots)} resource variants, {len(generator.definitions)} definitions")


if __name__ == "__main__":
    main()
