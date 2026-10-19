"""Conservative documentation qualification and required CI-result aggregation."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

ROOT_DOCS = {"README.md", "SECURITY.md", "AGENTS.md", "QUALITY_BAR.md"}
INDEX_DOCS = {"docs/BUILD_ORDER.md", "docs/ENGINEERING_STATE.md",
              "docs/MVP_AND_DEFERRED_SCOPE.md"}
DOC_PATTERN = r"docs/(?:adr|prd|contracts|build-order|workflows)/[A-Za-z0-9_./-]+\.md"
REQUIRED = {"changes", "links", "structure", "metrics", "rust", "codeql", "dependencies"}


def git(*args):
    return subprocess.run(["git", *args], check=True, capture_output=True,
                          timeout=30).stdout


def document(path):
    if ".." in path.split("/"):
        return False
    return path in ROOT_DOCS | INDEX_DOCS or re.fullmatch(DOC_PATTERN, path) is not None


def plain_file_at(revision, path):
    entry = git("ls-tree", "-z", revision, "--", path)
    # A deleted/new file may be absent at one end. Symlinks/submodules are full.
    return not entry or entry.startswith(b"100644 ")


def base_revision(event, payload):
    if event == "pull_request":
        return payload["pull_request"]["base"]["sha"]
    if event == "push":
        return payload["before"]
    return None


def classify(event, payload):
    base = base_revision(event, payload)
    if not isinstance(base, str) or not re.fullmatch(r"[0-9a-f]{40}", base):
        return "full"
    git("cat-file", "-e", base + "^{commit}")
    paths = git("diff", "--name-only", "--no-renames", "-z", base, "HEAD")
    names = paths.decode("utf-8").rstrip("\0").split("\0")
    if not names or not all(document(path) for path in names):
        return "full"
    for path in names:
        if not plain_file_at(base, path) or not plain_file_at("HEAD", path):
            return "full"
    return "docs"


def select_scope():
    try:
        payload = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
        return classify(os.environ.get("GITHUB_EVENT_NAME"), payload)
    except (KeyError, TypeError, ValueError, OSError, subprocess.SubprocessError):
        return "full"


def gate(needs):
    if not isinstance(needs, dict) or set(needs) != REQUIRED:
        return False
    scope = needs["changes"].get("outputs", {}).get("scope")
    if scope not in {"docs", "full"}:
        return False
    for name, data in needs.items():
        skipped_docs = name == "rust" and scope == "docs" and data.get("result") == "skipped"
        if data.get("result") != "success" and not skipped_docs:
            return False
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("classify", "gate"))
    args = parser.parse_args()
    if args.mode == "classify":
        scope = select_scope()
        with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
            output.write(f"scope={scope}\n")
        print(json.dumps({"scope": scope}))
        return 0
    try:
        accepted = gate(json.loads(os.environ["NEEDS"]))
    except (KeyError, TypeError, ValueError, AttributeError):
        accepted = False
    print("ci-ok OK" if accepted else "ci-ok FAILED: missing or ineligible job result")
    return 0 if accepted else 1


if __name__ == "__main__":
    raise SystemExit(main())
