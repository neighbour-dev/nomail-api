"""Guard the OpenAPI artifact and release prerequisites without extra packages."""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[2]


def jobs(path):
    """Read job blocks; needs in these workflows use inline lists or one job."""
    source = (ROOT / path).read_text(encoding="utf-8").split("\njobs:\n", 1)[1]
    return dict(
        re.findall(
            r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)",
            source,
            re.MULTILINE | re.DOTALL,
        )
    )


def needs(job):
    match = re.search(r"^    needs: (.+)$", job, re.MULTILINE)
    if match is None:
        return []
    return [part.strip() for part in match[1].strip("[]").split(",")]


def ancestors(all_jobs, name):
    result = set()
    for dependency in needs(all_jobs[name]):
        result.add(dependency)
        result.update(ancestors(all_jobs, dependency))
    return result


class OpenApiWorkflowTests(unittest.TestCase):
    def test_ci_generates_verifies_and_uploads_one_file_in_order(self):
        ci = jobs(".github/workflows/ci.yml")
        gate = ci["openapi"]
        generate = "cargo run --locked -p nomail-app --features openapi --example export-openapi -- docs/swagger.json"
        verify = "cargo test --locked -p nomail-app --features openapi --lib"
        self.assertEqual(gate.count(generate), 1)
        self.assertLess(gate.index(generate), gate.index(verify))
        self.assertLess(gate.index(verify), gate.index("uses: actions/upload-artifact@"))
        self.assertIn("name: swagger\n", gate)
        self.assertIn("path: docs/swagger.json\n", gate)
        self.assertIn("if-no-files-found: error", gate)
        self.assertNotIn("if:", gate)
        self.assertNotIn("continue-on-error:", gate)

    def test_test_jobs_require_and_download_the_verified_artifact(self):
        ci = jobs(".github/workflows/ci.yml")
        for name, test in [("check", "cargo test --locked"), ("workers", "run: npm test")]:
            with self.subTest(job=name):
                job = ci[name]
                self.assertIn("openapi", ancestors(ci, name))
                self.assertIn("uses: actions/download-artifact@", job)
                self.assertIn("name: swagger\n", job)
                self.assertIn("path: docs\n", job)
                self.assertLess(job.index("uses: actions/download-artifact@"), job.index(test))
                self.assertNotIn("--example export-openapi", job)

    def test_all_release_builds_and_publish_jobs_require_successful_checks(self):
        release = jobs(".github/workflows/release.yml")
        self.assertIn("uses: ./.github/workflows/ci.yml", release["check"])
        for name in ["build", "build-binaries", "publish", "release"]:
            with self.subTest(job=name):
                self.assertIn("check", ancestors(release, name))
                self.assertIn("release-info", ancestors(release, name))
                self.assertNotIn("if:", release[name])
                self.assertNotIn("continue-on-error:", release[name])

    def test_release_reuses_swagger_and_checksums_it_before_upload(self):
        release = jobs(".github/workflows/release.yml")
        job = release["release"]
        self.assertIn("name: swagger\n", job)
        self.assertIn("path: ${{ runner.temp }}/release-assets\n", job)
        checksum = "sha256sum *.tar.gz swagger.json > SHA256SUMS"
        self.assertLess(job.index("name: swagger\n"), job.index(checksum))
        self.assertLess(job.index(checksum), job.index("gh release upload"))
        self.assertNotIn("--example export-openapi", (ROOT / ".github/workflows/release.yml").read_text())
        self.assertNotIn("swagger", release["build-binaries"])

    def test_generated_json_is_ignored_and_not_copied_into_docker(self):
        self.assertIn("/docs/swagger.json", (ROOT / ".gitignore").read_text().splitlines())
        self.assertIn("docs", (ROOT / ".dockerignore").read_text().splitlines())
        dockerfile = (ROOT / "Dockerfile").read_text()
        self.assertNotRegex(dockerfile, r"(?mi)^COPY\s+.*(?:swagger|\bdocs\b)")
        self.assertNotIn("--features openapi", dockerfile)
        self.assertNotIn("export-openapi", dockerfile)


if __name__ == "__main__":
    unittest.main()
