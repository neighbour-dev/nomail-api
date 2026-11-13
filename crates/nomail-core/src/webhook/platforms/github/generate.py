#!/usr/bin/env python3
"""Generate detailed native GitHub webhook projections from a pinned official spec.

Explicit maintenance only: Cargo builds/tests never download specifications.
Existing hand-written fields and Rust accessors are retained; generated details
are flattened into their native JSON objects, not an extra JSON wrapper.
"""
import argparse
from collections import deque
import hashlib
import json
from pathlib import Path
import re
import subprocess
import urllib.request
from urllib.parse import urlsplit

COMMIT = "bd6df26d2a0104874fe5025a25ebcde356413364"
SHA256 = "9416f1144aee15971790d0de5388e7ad88a6fdad759fdb729bf94b8414bfb57f"
URL = f"https://raw.githubusercontent.com/github/rest-api-description/{COMMIT}/descriptions/api.github.com/api.github.com.json"
HERE = Path(__file__).parent
KEYWORDS = set("as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true try type unsafe use where while yield abstract become box do final macro override priv typeof unsized virtual gen".split())


def pascal(name):
    return "".join(part[:1].upper() + part[1:] for part in re.split(r"[^A-Za-z0-9]+", name) if part)


def field_name(key):
    special = {"+1": "plus_one", "-1": "minus_one", "self": "self_link", "super": "super_link", "crate": "crate_value"}
    name = special.get(key, re.sub(r"[^A-Za-z0-9_]", "_", key))
    if not name or name[0].isdigit():
        name = "field_" + name
    return "r#" + name if name in KEYWORDS else name


def legacy_models(text):
    result = {}
    pattern = r"model!\((\w+)(?:\s*\[(\w+):\s*(\w+)\])?\s*\+\s*(\w+)\s*\{(.*?)\}\);"
    for match in re.finditer(pattern, text, re.S):
        name, flat, base, details, body = match.groups()
        fields = {}
        field_pattern = r'(?:#\[serde\(rename\s*=\s*"([^"]+)"\)\]\s*)?(\w+):\s*(Option|Vec)<([^>]+)>'
        for prop in re.finditer(field_pattern, body):
            key, rust, container, ty = prop.groups()
            fields[key or rust] = (container, ty)
        result[name] = {"base": base, "details": details, "fields": fields}
    if not result or "GithubWebhookPayload" not in result:
        raise ValueError("Unable to read maintained GitHub model declarations")
    return result


class Generator:
    def __init__(self, spec, legacy):
        self.spec = spec
        self.schemas = spec["components"]["schemas"]
        self.legacy = legacy
        self.nodes = {}
        self.queue = deque()
        self.normalized = {}
        self.unions = {}
        self.operations = []
        for name in legacy:
            self.node(name)

    def node(self, name):
        if name not in self.nodes:
            if len(name) > 512 or len(self.nodes) >= 4096:
                raise ValueError("Unbounded model graph; review recursive resource sharing before generating")
            self.nodes[name] = {"fields": {}, "seen": set(), "objects": []}
        return self.nodes[name]

    def normalize(self, schema, stack=()):
        if not isinstance(schema, dict):
            raise ValueError(f"Unsupported schema value: {schema}")
        identity = id(schema)
        if identity in self.normalized:
            return self.normalized[identity][1]
        if "$ref" in schema:
            ref = schema["$ref"]
            prefix = "#/components/schemas/"
            if not ref.startswith(prefix) or ref in stack:
                raise ValueError(f"Unsupported/cyclic schema alias: {ref}")
            values = self.normalize(self.schemas[ref[len(prefix):]], (*stack, ref))
        elif any(key in schema for key in ("allOf", "oneOf", "anyOf")):
            # An intersection contributes all fields; an object union contributes
            # alternatives to the same tolerant native slot. No first-match DTO.
            values = []
            for key in ("allOf", "oneOf", "anyOf"):
                for branch in schema.get(key, []):
                    values.extend(self.normalize(branch, stack))
            # `type: object` alongside oneOf is a constraint, not a separate
            # arbitrary-object alternative. allOf object fragments describe one
            # object identity; combine them before canonical resource sharing.
            if "allOf" in schema and all(value["type"] in ("object", "null") for value in values):
                props = {}
                for branch in values:
                    for key, prop in branch.get("properties", {}).items():
                        props.setdefault(key, []).append(prop)
                for key, prop in schema.get("properties", {}).items():
                    props.setdefault(key, []).append(prop)
                merged = {key: options[0] if len(options) == 1 else {"anyOf": options} for key, options in props.items()}
                values = [{"type": "object", "properties": merged}] + ([{"type": "null"}] if any(value["type"] == "null" for value in values) else [])
            elif schema.get("properties"):
                values = [{**value, "properties": {**schema["properties"], **value.get("properties", {})}} if value["type"] == "object" else value for value in values]
            elif not values and "type" in schema:
                values = self.normalize({key: value for key, value in schema.items() if key not in ("allOf", "oneOf", "anyOf")}, stack)
        else:
            kind = schema.get("type")
            if isinstance(kind, list):
                values = [{**schema, "type": value} for value in kind]
            else:
                if kind is None and ("properties" in schema or "additionalProperties" in schema):
                    kind = "object"
                if kind is None and schema.get("enum"):
                    kinds = set("null" if value is None else "boolean" if isinstance(value, bool) else "integer" if isinstance(value, int) else "number" if isinstance(value, float) else "string" if isinstance(value, str) else "unsupported" for value in schema["enum"])
                    values = [{**schema, "type": value} for value in sorted(kinds)]
                elif kind is None and "required" in schema:
                    values = []  # allOf presence constraint, not an arbitrary value.
                else:
                    values = [{**schema, "type": kind or "any"}]
        if schema.get("nullable") and not any(value["type"] == "null" for value in values):
            values = [*values, {"type": "null"}]
        # Retain the input too: temporary allOf/type-list dictionaries must not
        # be collected and have their Python id reused for a different schema.
        self.normalized[identity] = (schema, values)
        return values

    def register(self, name, choices):
        node = self.node(name)
        for schema in choices:
            for branch in self.normalize(schema):
                if branch["type"] != "object":
                    continue
                identity = id(branch)
                if identity not in node["seen"]:
                    node["seen"].add(identity)
                    node["objects"].append(branch)
                    self.queue.append((name, branch))

    def context(self, parent, key):
        prefix = "Github" if parent == "GithubWebhookPayload" else parent
        return prefix + pascal(key)

    def canonical(self, branch):
        props = set(branch.get("properties", {}))
        signatures = [
            ({"inviter", "invitation_teams_url"}, "GithubInvitation"),
            ({"slug", "website_url"}, "GithubEnterprise"),
            ({"is_answerable", "emoji"}, "GithubDiscussionCategory"),
            ({"full_name", "owner"}, "GithubRepository"),
            ({"head", "base", "number"}, "GithubPullRequest"),
            ({"login", "members_url", "public_members_url"}, "GithubOrganization"),
            ({"login", "id"}, "GithubUser"),
            ({"tag_name", "assets"}, "GithubRelease"),
            ({"browser_download_url", "download_count"}, "GithubReleaseAsset"),
            ({"due_on", "open_issues", "closed_issues"}, "GithubMilestone"),
            ({"number", "labels", "title"}, "GithubIssue"),
            ({"slug", "privacy", "members_url"}, "GithubTeam"),
            ({"color", "default", "name"}, "GithubLabel"),
            ({"slug", "external_url", "permissions"}, "GithubApp"),
            ({"total_count", "+1", "-1"}, "GithubReactions"),
            ({"steps", "run_id", "runner_id"}, "GithubWorkflowJob"),
            ({"head_sha", "run_attempt", "workflow_id"}, "GithubWorkflowRun"),
            ({"path", "badge_url"}, "GithubWorkflow"),
            ({"output", "check_suite", "started_at"}, "GithubCheckRun"),
            ({"latest_check_runs_count", "check_runs_url"}, "GithubCheckSuite"),
            ({"category", "answer_html_url"}, "GithubDiscussion"),
            ({"commit_id", "submitted_at", "state"}, "GithubReview"),
            ({"sha", "environment", "task"}, "GithubDeployment"),
            ({"environment_url", "state", "deployment_url"}, "GithubDeploymentStatus"),
            ({"id", "message", "timestamp"}, "GithubCommit"),
            ({"sha", "commit", "parents"}, "GithubStatusCommit"),
            ({"message", "tree", "comment_count"}, "GithubGitCommit"),
        ]
        for required, name in signatures:
            if required <= props:
                return name
        if {"name", "email"} <= props and props <= {"name", "email", "date", "username"}:
            return "GithubCommitAuthor"
        if {"ref", "sha"} <= props and props & {"repo", "label", "user"}:
            return "GithubBranch"
        return None

    def object_type(self, choices, context):
        structured = [branch for branch in choices if branch.get("properties")]
        if not structured:
            values = []
            for branch in choices:
                additional = branch.get("additionalProperties", True)
                if isinstance(additional, dict):
                    values.append(additional)
                elif additional is not False:
                    return "BTreeMap<String, Value>"
            if values:
                return f"BTreeMap<String, {self.infer(values, context + 'Value', item=True)}>"
            return "BTreeMap<String, Value>"
        if len(structured) != len(choices):
            # An explicitly open object alternative cannot be narrowed to just
            # another branch's named fields. Preserve the whole native object.
            return "BTreeMap<String, Value>"
        names = {self.canonical(branch) for branch in structured}
        name = next(iter(names)) if len(names) == 1 and None not in names else context
        self.register(name, structured)
        return f"Box<{name}>"

    def infer(self, choices, context, item=False):
        branches = [branch for schema in choices for branch in self.normalize(schema)]
        nullable = any(branch["type"] == "null" for branch in branches)
        branches = [branch for branch in branches if branch["type"] != "null"]
        kinds = {branch["type"] for branch in branches}
        if not kinds or "any" in kinds:
            ty = "Value"
        else:
            unknown = kinds - {"object", "array", "integer", "number", "string", "boolean"}
            if unknown:
                raise ValueError(f"Unsupported schema kinds at {context}: {unknown}")
            if "number" in kinds:
                kinds.discard("integer")  # Number keeps integer JSON values too.
            variants = []
            for kind in sorted(kinds):
                schemas = [branch for branch in branches if branch["type"] == kind]
                if kind == "object":
                    field_ty = self.object_type(schemas, context)
                elif kind == "array":
                    items = [schema.get("items", {}) for schema in schemas]
                    field_ty = f"Vec<{self.infer(items, context + 'Item', item=True)}>"
                else:
                    field_ty = {"integer": "i64", "number": "GithubNumber", "string": "String", "boolean": "bool"}[kind]
                variants.append((kind, field_ty))
            if len(variants) == 1:
                ty = variants[0][1]
            else:
                name = context + "Value"
                identity = tuple(variants)
                # During fixed-point discovery a slot can acquire branches.
                self.unions[name] = identity
                ty = f"Box<{name}>"
        return f"Option<{ty}>" if nullable and item and ty != "Value" else ty

    def maintained_child(self, binding, choices):
        container, ty = binding
        schemas = choices
        if container == "Vec":
            schemas = [branch.get("items", {}) for schema in schemas for branch in self.normalize(schema) if branch["type"] == "array"]
        if ty in self.legacy:
            self.register(ty, schemas)
        elif ty in {"GithubLabelValue", "GithubCommentValue", "GithubWorkflowValue"}:
            target = {"GithubLabelValue": "GithubLabel", "GithubCommentValue": "GithubComment", "GithubWorkflowValue": "GithubWorkflow"}[ty]
            self.register(target, schemas)

    def discover(self):
        for name, item in sorted(self.spec["x-webhooks"].items()):
            operation = item["post"]
            content = operation["requestBody"]["content"]["application/json"]
            event = operation["x-github"]["subcategory"]
            # The source's example headers include known typos (discussions,
            # project-v2). Its canonical subcategory/docs anchor is authoritative.
            self.operations.append((name, event))
            self.register("GithubWebhookPayload", [content["schema"]])
        while self.queue:
            name, branch = self.queue.popleft()
            node = self.nodes[name]
            for key, schema in branch.get("properties", {}).items():
                values = node["fields"].setdefault(key, {})
                values[id(schema)] = schema
            for key, schemas in node["fields"].items():
                schemas = list(schemas.values())
                binding = self.binding(name, key)
                if binding:
                    self.maintained_child(binding, schemas)
                else:
                    self.infer(schemas, self.context(name, key))

    def binding(self, name, key):
        model = self.legacy.get(name)
        if not model:
            return None
        return model["fields"].get(key) or (self.binding(model["base"], key) if model["base"] else None)

    def covered_fields(self, name):
        model = self.legacy.get(name)
        if not model:
            return set()
        keys = set(model["fields"])
        if model["base"]:
            keys |= set(self.nodes[model["base"]]["fields"]) | self.covered_fields(model["base"])
        return keys

    def render(self):
        self.discover()
        parts = [f'''//! Generated by generate.py; DO NOT EDIT.
//! Official github/rest-api-description commit {COMMIT}.
//! Source SHA-256: {SHA256}; MIT notice in GITHUB-LICENSE.txt.
//! Native read projections across webhook actions, not strict per-event validators.

use super::models::*;
use super::support::*;
use serde_json::Value;
use std::collections::BTreeMap;

pub const GITHUB_MODEL_SPEC_COMMIT: &str = {json.dumps(COMMIT)};
pub const GITHUB_MODEL_OPERATION_COUNT: usize = {len(self.operations)};
pub const GITHUB_MODEL_EVENT_COUNT: usize = {len(set(event for _, event in self.operations))};
''']
        node_count = len(self.nodes)
        for name, node in sorted(self.nodes.items()):
            output_name = self.legacy[name]["details"] if name in self.legacy else name
            if output_name != name and output_name in self.nodes:
                raise ValueError(f"Generated/maintained model name collision: {output_name}")
            parts.append(f"github_model!({output_name} {{")
            rust_names = set()
            for key, choices in sorted(node["fields"].items()):
                if key in self.covered_fields(name):
                    continue
                rust = field_name(key)
                if rust in rust_names:
                    raise ValueError(f"Field identifier collision at {name}.{key}")
                rust_names.add(rust)
                ty = self.infer(list(choices.values()), self.context(name, key))
                parts.append(f"    {rust}: {ty} = {json.dumps(key)},")
            parts.append("});\n")
        if len(self.nodes) != node_count or self.queue:
            raise ValueError("Model discovery did not reach a fixed point")
        methods = {"array": "is_array", "object": "is_object", "integer": "is_i64", "number": "is_number", "string": "is_string", "boolean": "is_boolean"}
        for name, variants in sorted(self.unions.items()):
            if name in self.nodes or name in self.legacy:
                raise ValueError(f"Union/model name collision: {name}")
            parts.append(f"github_union!({name} {{")
            for kind, ty in variants:
                parts.append(f"    {pascal(kind)}({ty}) if |value: &Value| value.{methods[kind]}(),")
            parts.append("});\n")
        parts += ["#[cfg(test)]", "pub(super) const WEBHOOK_OPERATIONS: &[(&str, &str)] = &["]
        parts += [f"    ({json.dumps(name)}, {json.dumps(event)})," for name, event in self.operations]
        parts.append("];\n")
        return "\n".join(parts)

    def sample(self, schema, depth=0, key=""):
        """Bounded synthetic type probes, NOT complete real/upstream-valid events."""
        branches = [branch for branch in self.normalize(schema) if branch["type"] != "null"]
        if not branches:
            return None
        kind = branches[0]["type"]
        same = [branch for branch in branches if branch["type"] == kind]
        if kind == "object":
            if depth >= 3:
                return {}
            properties = {}
            for branch in same:
                for key, value in branch.get("properties", {}).items():
                    properties.setdefault(key, []).append(value)
            if properties:
                return {key: self.sample({"anyOf": values}, depth + 1, key) for key, values in sorted(properties.items())}
            additional = next((branch["additionalProperties"] for branch in same if isinstance(branch.get("additionalProperties"), dict)), None)
            return {"example": self.sample(additional, depth + 1)} if additional is not None else {}
        if kind == "array":
            return [] if depth >= 3 else [self.sample(same[0].get("items", {}), depth + 1)]
        for branch in same:
            values = [value for value in branch.get("enum", []) if value is not None]
            if values:
                return values[0]
        if kind == "string":
            formats = {branch.get("format") for branch in same}
            if "date-time" in formats:
                return "2026-11-13T01:02:03Z"
            if "uri" in formats or "url" in formats:
                return "https://example.invalid/example"
            if "email" in formats:
                return "developer@example.invalid"
            return "example"
        if kind == "integer":
            return 7
        if kind == "number":
            # Several official identifier schemas say number rather than
            # integer. Use realistic integral IDs, not imaginary fractional IDs.
            return 7 if key == "id" or key.endswith("_id") else 1.25
        if kind == "boolean":
            return False
        if kind == "any":
            # No declared fields to probe. Do not invent a nested property and
            # then claim that every maintained compatibility model declares it.
            return None
        raise ValueError(f"Cannot create a fixture for {kind}")

    def fixtures(self):
        grouped, official = {}, []
        for name, item in sorted(self.spec["x-webhooks"].items()):
            operation = item["post"]
            content = operation["requestBody"]["content"]["application/json"]
            event = operation["x-github"]["subcategory"]
            grouped.setdefault(event, []).append(content["schema"])
            for example in content.get("examples", {}).values():
                if "$ref" in example:
                    prefix = "#/components/examples/"
                    if not example["$ref"].startswith(prefix):
                        raise ValueError("Unsupported fixture example reference")
                    example = self.spec["components"]["examples"][example["$ref"][len(prefix):]]
                if "value" in example:
                    official.append({"name": name, "event": event, "payload": example["value"]})
        probes = [{"name": event + "-schema-probe", "event": event, "payload": self.sample({"anyOf": schemas})} for event, schemas in sorted(grouped.items())]
        return {"operations.json": sanitize(probes), "official.json": sanitize(official)}


def sanitize(value, key=""):
    if isinstance(value, dict):
        return {name: sanitize(item, name) for name, item in value.items()}
    if isinstance(value, list):
        return [sanitize(item, key) for item in value]
    if isinstance(value, str):
        if key == "secret" or key.endswith("_token"):
            return "example_not_a_credential"
        if "email" in key and "@" in value:
            return "developer@example.invalid"
        if value.startswith(("https://", "http://")):
            return "https://example.invalid" + urlsplit(value).path
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema", type=Path, help="Use an already downloaded pinned specification")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--fixtures", action="store_true", help="Generate/check bounded synthetic probes and sanitized official examples")
    args = parser.parse_args()
    data = args.schema.read_bytes() if args.schema else urllib.request.urlopen(URL, timeout=90).read()
    if hashlib.sha256(data).hexdigest() != SHA256:
        raise ValueError("Specification SHA-256 mismatch; review upstream before changing pins")
    spec = json.loads(data)
    legacy = legacy_models((HERE / "models.rs").read_text())
    generator = Generator(spec, legacy)
    output = generator.render()
    output = subprocess.run(["rustfmt", "--edition", "2024"], input=output, text=True, capture_output=True, check=True).stdout
    path = HERE / "generated.rs"
    if args.check:
        if path.read_text() != output:
            raise SystemExit("GitHub generated.rs is stale")
    else:
        path.write_text(output)
    if args.fixtures:
        for name, value in generator.fixtures().items():
            path = HERE / "fixtures" / name
            text = json.dumps(value, indent=2, sort_keys=True) + "\n"
            if args.check:
                if path.read_text() != text:
                    raise SystemExit(f"GitHub {name} is stale")
            else:
                path.parent.mkdir(exist_ok=True)
                path.write_text(text)
    print(f"GitHub: {len(generator.operations)} operation schemas, {len(set(event for _, event in generator.operations))} event names, {len(generator.nodes)} models, {len(generator.unions)} kind unions")


if __name__ == "__main__":
    main()
