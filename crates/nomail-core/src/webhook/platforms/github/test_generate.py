"""Offline generator tests; no network or third-party Python dependencies."""
import unittest

from generate import Generator, field_name, legacy_models, sanitize


def spec(properties, schemas=None, event="issues", example_event="issues"):
    return {
        "components": {"schemas": schemas or {}, "examples": {}},
        "x-webhooks": {"example": {"post": {
            "x-github": {"subcategory": event},
            "parameters": [{"name": "X-GitHub-Event", "example": example_event}],
            "requestBody": {"content": {"application/json": {"schema": {"type": "object", "properties": properties}}}},
        }}},
    }


def maintained(fields=None):
    return {"GithubWebhookPayload": {"base": None, "details": "GithubWebhookPayloadDetails", "fields": fields or {}}}


class GeneratorTests(unittest.TestCase):
    def test_allof_adds_fields_without_weakening_required_only_fragments(self):
        schema = spec({"thing": {"allOf": [
            {"$ref": "#/components/schemas/base"},
            {"type": "object", "properties": {"enabled": {"type": "boolean"}}},
            {"required": ["enabled"]},
        ]}}, {"base": {"type": "object", "properties": {"name": {"type": "string"}}}})
        output = Generator(schema, maintained()).render()
        self.assertIn('enabled: bool = "enabled"', output)
        self.assertIn('name: String = "name"', output)
        self.assertNotIn('thing: Value', output)

    def test_ambiguous_object_unions_merge_native_fields(self):
        schema = spec({"reviewer": {"oneOf": [
            {"type": "object", "properties": {"login": {"type": "string"}}},
            {"type": "object", "properties": {"slug": {"type": "string"}}},
        ]}})
        output = Generator(schema, maintained()).render()
        self.assertIn('login: String = "login"', output)
        self.assertIn('slug: String = "slug"', output)
        self.assertNotIn('github_union!', output)

    def test_object_type_constraint_is_not_an_open_oneof_alternative(self):
        schema = spec({"thing": {"type": "object", "oneOf": [
            {"type": "object", "properties": {"a": {"type": "string"}}},
            {"type": "object", "properties": {"b": {"type": "boolean"}}},
        ]}})
        output = Generator(schema, maintained()).render()
        self.assertIn('a: String = "a"', output)
        self.assertIn('b: bool = "b"', output)
        self.assertNotIn('thing: BTreeMap', output)

    def test_invitation_is_not_shared_as_a_user_just_because_it_has_login_and_id(self):
        gen = Generator(spec({}), maintained())
        self.assertEqual(gen.canonical({"properties": {key: {} for key in ["login", "id", "inviter", "invitation_teams_url"]}}), "GithubInvitation")

    def test_mixed_native_wire_kinds_have_an_untagged_union(self):
        schema = spec({"value": {"oneOf": [{"type": "string"}, {"type": "integer"}]}})
        output = Generator(schema, maintained()).render()
        self.assertIn('String(String)', output)
        self.assertIn('Integer(i64)', output)
        self.assertIn('value.is_string()', output)
        self.assertIn('value.is_i64()', output)

    def test_nullable_array_elements_and_typed_maps_remain_distinct(self):
        schema = spec({
            "items": {"type": "array", "items": {"type": "string", "nullable": True}},
            "flags": {"type": "object", "additionalProperties": {"type": "boolean"}},
        })
        output = Generator(schema, maintained()).render()
        self.assertIn('items: Vec<Option<String>>', output)
        self.assertIn('flags: BTreeMap<String, bool>', output)

    def test_open_object_alternative_is_not_narrowed_to_another_branch(self):
        schema = spec({"payload": {"oneOf": [
            {"type": "object", "properties": {"one": {"type": "string"}}},
            {"type": "object", "additionalProperties": True},
        ]}})
        self.assertIn('payload: BTreeMap<String, Value>', Generator(schema, maintained()).render())

    def test_existing_accessors_are_not_duplicated_in_flattened_details(self):
        schema = spec({"action": {"type": "string"}, "new_field": {"type": "boolean"}})
        output = Generator(schema, maintained({"action": ("Option", "String")})).render()
        self.assertNotIn('action: String', output)
        self.assertIn('new_field: bool', output)

    def test_canonical_event_metadata_wins_over_incorrect_example_headers(self):
        gen = Generator(spec({}, event="projects_v2", example_event="project-v2"), maintained())
        gen.render()
        self.assertEqual(gen.operations, [("example", "projects_v2")])

    def test_recursive_repository_sources_reach_a_fixed_point(self):
        repo = {"type": "object", "properties": {
            "full_name": {"type": "string"}, "owner": {"type": "object", "properties": {
                "login": {"type": "string"}, "id": {"type": "integer"}}},
            "parent": {"$ref": "#/components/schemas/repo"},
        }}
        gen = Generator(spec({"repository": {"$ref": "#/components/schemas/repo"}}, {"repo": repo}), maintained())
        output = gen.render()
        self.assertLess(len(gen.nodes), 10)
        self.assertIn('parent: Box<GithubRepository>', output)

    def test_model_parser_keeps_native_keys_and_existing_flattening(self):
        parsed = legacy_models('''model!(GithubWebhookPayload + GithubWebhookPayloadDetails {
            #[serde(rename = "ref")] ref_name: Option<String>, items: Vec<GithubUser>,
        });
        model!(GithubChild [base: GithubWebhookPayload] + GithubChildDetails { id: Option<u64>, });''')
        self.assertEqual(parsed["GithubWebhookPayload"]["fields"]["ref"], ("Option", "String"))
        self.assertEqual(parsed["GithubChild"]["base"], "GithubWebhookPayload")

    def test_sanitization_and_identifier_mapping(self):
        self.assertEqual(sanitize({"secret": "placeholder", "email": "a@example.com"}),
                         {"secret": "example_not_a_credential", "email": "developer@example.invalid"})
        self.assertEqual(field_name("type"), "r#type")
        self.assertEqual(field_name("self"), "self_link")
        self.assertEqual(field_name("+1"), "plus_one")

    def test_unknown_reference_or_schema_kind_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "Unsupported"):
            Generator(spec({"thing": {"$ref": "https://example.invalid/schema"}}), maintained()).render()
        with self.assertRaisesRegex(ValueError, "Unsupported schema kinds"):
            Generator(spec({"thing": {"type": "invented"}}), maintained()).render()

    def test_probe_examples_are_bounded_and_use_only_synthetic_values(self):
        gen = Generator(spec({"foo": {"type": "string"}, "flag": {"type": "boolean"}}), maintained())
        probes = gen.fixtures()["operations.json"]
        self.assertEqual(probes[0]["payload"], {"flag": False, "foo": "example"})
        self.assertIsNone(gen.sample({}))
        self.assertEqual(gen.sample({"type": "number"}, key="run_id"), 7)
        self.assertEqual(gen.sample({"type": "number"}, key="score"), 1.25)


if __name__ == "__main__":
    unittest.main()
