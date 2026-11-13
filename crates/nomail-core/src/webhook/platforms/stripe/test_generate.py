"""Offline regression tests for the pinned-spec Rust generator."""
import unittest

from generate import Generator, field_name, fixture_subset


def object_schema(tag, **properties):
    return {"type": "object", "properties": {"object": {"type": "string", "enum": [tag]}, **properties}}


def generator(**schemas):
    return Generator({"components": {"schemas": schemas}})


class GeneratorTests(unittest.TestCase):
    def test_recursive_expandables_are_boxed_and_shared(self):
        gen = generator(customer=object_schema("customer", parent={"anyOf": [
            {"type": "string"}, {"$ref": "#/components/schemas/customer"}
        ]}))
        output = gen.generate()
        self.assertEqual(output.count("stripe_model!(StripeCustomer ["), 1)
        self.assertIn("Customer(Box<StripeCustomer>)", output)
        self.assertIn("String(String)", output)
        self.assertIn('value.get("object")', output)
        self.assertIn('stripe_tag!(StripeCustomerObjectTag { Customer = "customer" })', output)

    def test_untagged_server_client_objects_merge_without_guessing(self):
        gen = generator(
            sample=object_schema("sample", options={"anyOf": [
                {"$ref": "#/components/schemas/server"}, {"$ref": "#/components/schemas/client"}
            ]}),
            server={"type": "object", "properties": {"server_only": {"type": "boolean"}}},
            client={"type": "object", "properties": {"client_only": {"type": "integer"}}},
        )
        output = gen.generate()
        self.assertIn('server_only: bool = "server_only"', output)
        self.assertIn('client_only: i64 = "client_only"', output)
        self.assertIn("stripe_model!(StripeSampleOptionsValue", output)

    def test_native_numeric_dictionary_and_nullable_collection_types(self):
        gen = generator(sample=object_schema("sample",
            decimal={"type": "string"}, number={"type": "number"}, amount={"type": "integer"},
            values={"type": "array", "items": {"type": "string", "nullable": True}},
            metadata={"type": "object", "additionalProperties": {"type": "string"}},
        ))
        output = gen.generate()
        for expected in ("decimal: String", "number: StripeNumber", "amount: i64",
                         "values: Vec<Option<String>>", "metadata: BTreeMap<String, String>"):
            self.assertIn(expected, output)

    def test_unhandled_schema_constructs_fail_instead_of_becoming_json_values(self):
        gen = generator(sample=object_schema("sample", unsupported={"allOf": []}))
        with self.assertRaisesRegex(ValueError, "Unsupported schema"):
            gen.generate()

    def test_duplicate_discriminators_and_rust_name_collisions_fail(self):
        with self.assertRaisesRegex(ValueError, "Ambiguous resource tag"):
            generator(one=object_schema("duplicate"), two=object_schema("duplicate")).generate()
        with self.assertRaisesRegex(ValueError, "Rust name collision"):
            generator(**{"a.b": object_schema("a"), "a_b": object_schema("b")}).generate()

    def test_deleted_resource_dispatch_is_distinct_from_live_resource(self):
        output = generator(customer=object_schema("customer"), deleted_customer=object_schema(
            "customer", deleted={"type": "boolean", "enum": [True]}
        )).generate()
        self.assertIn("Customer(Box<StripeCustomer>)", output)
        self.assertIn("DeletedCustomer(Box<StripeDeletedCustomer>)", output)
        self.assertIn('("customer", true)', output)
        self.assertIn('("customer", false)', output)

    def test_fixture_integrity_and_keywords(self):
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            fixture_subset(b"unreviewed fixture source")
        self.assertEqual(field_name("type"), "r#type")
        self.assertEqual(field_name("gen"), "r#gen")
        self.assertEqual(field_name("amount"), "amount")


if __name__ == "__main__":
    unittest.main()
