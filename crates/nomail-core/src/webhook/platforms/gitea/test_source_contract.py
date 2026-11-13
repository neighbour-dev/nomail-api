"""Offline tests for the Gitea wire-contract checker; no Go/network needed."""
import unittest

from source_contract import rust_models, validate_models


def contract(fields, types=None):
    return {"events": [{"payload_type": "Payload"}], "types": {
        "Payload": {"fields": {key: {"type": value} for key, value in fields.items()}},
        **(types or {}),
    }}


class ContractTests(unittest.TestCase):
    def test_balanced_rust_fields_keep_nested_options_boxes_and_native_names(self):
        parsed = rust_models('''model!(GiteaWebhookPayload {
            #[serde(rename = "ref")] ref_name: Option<String>,
            parent: Option<Box<GiteaRepository>>, users: Vec<Option<GiteaUser>>,
            units_map: BTreeMap<String, String>,
        });''')
        self.assertEqual(parsed["GiteaWebhookPayload"], {
            "ref": "String", "parent": "Box<GiteaRepository>",
            "users": "Vec<Option<GiteaUser>>", "units_map": "BTreeMap<String,String>",
        })

    def test_missing_custom_serializer_alias_is_rejected(self):
        c = contract({"username": {"named": "string"}})
        with self.assertRaisesRegex(ValueError, "username: missing field"):
            validate_models(c, {"GiteaWebhookPayload": {}})
        self.assertEqual(validate_models(c, {"GiteaWebhookPayload": {"username": "String"}}), 1)

    def test_pointer_slice_members_require_nullable_elements(self):
        c = contract({"users": {"slice": {"pointer": {"named": "User"}}}}, {
            "User": {"fields": {"id": {"type": {"named": "int64"}}}},
        })
        models = {"GiteaWebhookPayload": {"users": "Vec<GiteaUser>"}, "GiteaUser": {"id": "i64"}}
        with self.assertRaisesRegex(ValueError, "must retain null"):
            validate_models(c, models)
        models["GiteaWebhookPayload"]["users"] = "Vec<Option<GiteaUser>>"
        self.assertEqual(validate_models(c, models), 2)

    def test_scalar_and_dictionary_types_are_not_coerced(self):
        c = contract({"units": {"map": {"named": "string"}}, "id": {"named": "int64"}})
        valid = {"GiteaWebhookPayload": {"units": "BTreeMap<String,String>", "id": "i64"}}
        self.assertEqual(validate_models(c, valid), 1)
        invalid = {"GiteaWebhookPayload": {"units": "BTreeMap<String,bool>", "id": "String"}}
        with self.assertRaisesRegex(ValueError, "does not match"):
            validate_models(c, invalid)

    def test_organization_slot_checks_the_actual_user_shape_not_just_the_name(self):
        c = contract({"organization": {"pointer": {"named": "User"}}}, {
            "User": {"fields": {"is_admin": {"type": {"named": "bool"}}, "username": {"type": {"named": "string"}}}},
        })
        models = {"GiteaWebhookPayload": {"organization": "GiteaOrganization"}, "GiteaOrganization": {"username": "String"}}
        with self.assertRaisesRegex(ValueError, "is_admin: missing field"):
            validate_models(c, models)
        models["GiteaOrganization"]["is_admin"] = "bool"
        self.assertEqual(validate_models(c, models), 2)

    def test_native_string_aliases_timestamps_and_wiki_comment_union(self):
        c = contract({"state": {"named": "StateType"}, "created_at": {"pointer": {"scalar": "string"}}, "comment": {"named": "string"}}, {
            "StateType": {"alias": {"named": "string"}},
        })
        models = {"GiteaWebhookPayload": {"state": "String", "created_at": "String", "comment": "GiteaWebhookComment"}}
        self.assertEqual(validate_models(c, models), 1)

    def test_recursive_repositories_are_checked_once_without_losing_fields(self):
        c = contract({"repository": {"pointer": {"named": "Repository"}}}, {
            "Repository": {"fields": {"id": {"type": {"named": "int64"}}, "parent": {"type": {"pointer": {"named": "Repository"}}}}},
        })
        models = {"GiteaWebhookPayload": {"repository": "GiteaRepository"}, "GiteaRepository": {"id": "i64", "parent": "Box<GiteaRepository>"}}
        self.assertEqual(validate_models(c, models), 2)

    def test_unknown_referenced_types_fail_instead_of_becoming_arbitrary_json(self):
        with self.assertRaisesRegex(ValueError, "unknown Go type"):
            validate_models(contract({"field": {"named": "Unreviewed"}}), {"GiteaWebhookPayload": {"field": "String"}})


if __name__ == "__main__":
    unittest.main()
