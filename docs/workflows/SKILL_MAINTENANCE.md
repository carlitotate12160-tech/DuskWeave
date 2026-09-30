# DuskWeave skill maintenance

## Source and roles

Maintain workflow sources in .agents/skills/ in this repository.
Keep engineering, build and adversarial-review responsibilities separate.
Keep short offensive/domain anchors loaded; use PRDs/ADRs for full semantics,
AGENTS.md for contributor protocol and QUALITY_BAR.md for assurance rules.
Never store current stage, seal or working HEAD in portable skill instructions.
Check component-specific language authority instead of copying an abbreviated
language policy that misclassifies Rust infrastructure ports.

## Change and publication

1. Verify base/head, authority and unrelated work. Use an isolated worktree when
   an implementation branch is active. State the exact documentation edit map.
2. Update the source skills and packet template together when their contract changes.
   Keep source links and package-local resources valid.
3. Run document links/structure/diff checks. Validate installed skill frontmatter
   and inspect existing UI metadata for consistency.
4. Exercise realistic bounded cases in a fresh context: packet feasibility,
   actual failure-test evidence, current-head CI and preserved offensive intent.
   Supply artifacts and task constraints without supplying the desired verdict.
   Static text checks cannot prove semantic compliance.
5. Commit reviewed repository source on its delivery branch. For installed copies,
   publish each existing skill through its supported update mechanism; preserve
   its identity and package-local resources. Record the source commit in the
   update record, including whether it is an unmerged candidate.
6. Compare source and installed SKILL.md/template bytes after publication.
   Verify the persisted version, not only an edited local copy. Report unavailable
   installations as NOT UPDATED; do not claim a whole-machine/global deployment.
7. Keep user-facing active prompts consistent with current authority and candidate
   state. Mark completed/replaced implementation prompts historical; do not rerun
   obsolete coding work after a new candidate appears.

This is a publication procedure, not a new governance service. Do not add a
runtime manager, remote synchronization daemon or separate product model.
A local source/installed hash comparison verifies this publication only; repeat
it when either copy changes. Installed copies never override repository authority.

## Verification evidence

Record source SHA, installed skill names, equality check and actual validation.
Separate author self-review from an independently executed review/forward test.
Retain concrete findings and limits; do not call a documentation test a runtime test.
If current source conflicts with accepted authority, resolve that conflict before
publication. Do not modify PRD acceptance or historical seals as a side effect.

## Quality target

Use requirement, owner, failure boundary, assertion, environment and SHA as the
basis for assurance. Skills guide work; actual configured CI/branch checks enforce
their respective controls. Recheck live configuration when claiming enforcement.
No skill certifies enterprise readiness or military compliance.
Preserve authorized adversary emulation, active validation and low unnecessary
footprint; quality controls do not convert DuskWeave into a defensive-only product.
