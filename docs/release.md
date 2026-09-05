# Release a verified candidate

A tag push is the release action. `release.yml` checks the tag against Cargo.toml
and Cargo.lock, calls the complete existing CI workflow from the same commit,
then builds Linux x86_64, macOS arm64, macOS x86_64, and Windows x86_64 binaries.
No binary build starts until the CI jobs pass. The Linux build also runs
`cargo package --locked --no-verify`; portable tests compare Cargo metadata,
the compiled CLI version, and the package's required source files.

Before tagging:

- Record fresh Windows Office proof for the candidate using the interactive
  [Legion procedure](proof-windows.md). This is still required for release
  compatibility claims; Linux SDK and LibreOffice checks are insufficient.
- Copy the real `target/legion-proof/summary.json` into
  `proof/legion-summary.json` and commit it before the tag. Record the tested
  candidate SHA and Office build in the release-readiness document. Confirm
  that any subsequent commit only records proof and does not alter the product.
  The pipeline attaches this report if present. It rejects fixture-only,
  failed, and `SkipOffice` reports. Absence is stated in the generated notes;
  absence does not turn hosted CI into desktop Office proof.
- Have RoseCanyon finish the batch gates and commit-flush the closed beads.
  Release notes read the committed `.beads/issues.jsonl`, not a local database.
  `batch_pending` and open beads never appear as delivered work.
- Run the branch dry run below and inspect its exact SHA, green CI jobs, and
  complete candidate artifact. Make sure the intended release version matches
  Cargo.toml and Cargo.lock and that the tag does not already exist.
- Obtain the user's release go-ahead. Only the orchestrator pushes the tag.

For the current `0.1.0` package, run these commands from the verified candidate:

```bash
git tag -a v0.1.0 -m 'ooxml-cli v0.1.0'
git push origin refs/tags/v0.1.0
```

The push starts the pipeline and publishes the GitHub Release only after all
checks and asset assembly pass. No manual asset upload is needed. Do not move
or replace a published tag. A failed job can be rerun against the same tag;
a product fix requires a new reviewed candidate and version decision.

## Branch dry run

After the workflow is available on the default branch, the orchestrator can run:

```bash
gh workflow run release.yml --ref master
```

Substitute a pushed candidate branch for `master` when rehearsing a change.
Every `workflow_dispatch` is a dry run, including a dispatch against a tag.
The `publish` job requires a tag **push** event, so manual dispatch cannot
create or update a GitHub Release. Dry runs execute the same full CI workflow,
all four builds, source packaging, metadata generation, and checksum checks.
The Actions run retains `release-candidate-v0.1.0` for inspection.

## Assets

The complete candidate and published release contain:

- `ooxml-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`
- `ooxml-v0.1.0-aarch64-apple-darwin.tar.gz`
- `ooxml-v0.1.0-x86_64-apple-darwin.tar.gz`
- `ooxml-v0.1.0-x86_64-pc-windows-msvc.zip`
- `capabilities.json`, emitted by the packaged Linux binary after verifying its version
- `CHANGELOG.md`, generated from newly closed bead IDs since the previous release tag
- `legion-proof.json`, when `proof/legion-summary.json` is present and valid
- `SHA256SUMS`, covering every file above, checked after assembly and before publication

For the first release, the changelog uses all closed beads in the committed
snapshot. Subsequent releases compare that snapshot with the nearest earlier
`v[0-9]*` tag. The script refuses an unreadable previous snapshot rather than
quietly inventing release history. Versions in filenames follow Cargo's version;
branch names never become asset filenames.

To inspect downloaded files on Linux:

```bash
sha256sum --check SHA256SUMS
```

Local metadata tests run with:

```bash
python3 -m unittest discover -s tools -p test_release_assets.py -v
cargo test --test release_package
```

A local pass checks implementation and packaging contracts. Only an actual
hosted dry run establishes that the GitHub runner matrix and artifact transfers
work for the selected SHA. No hosted run or tag is implied by this document.
