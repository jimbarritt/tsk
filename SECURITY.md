# Security

## Supported versions

The latest release receives security fixes.

## Reporting a vulnerability

Report a vulnerability through GitHub private vulnerability reporting: open the Security tab of this repository and select "Report a vulnerability". The report is visible to the maintainer only. Do not open a public issue for a vulnerability.

## Verifying a release

Every release archive has a GitHub attestation. It states that GitHub Actions built the archive from this repository, in the workflow `binaries.yml`, and names the commit.

```bash
gh attestation verify tsk-<version>-<target>.tar.gz --repo jimbarritt/tsk --signer-workflow jimbarritt/tsk/.github/workflows/binaries.yml
```

`gh attestation verify` needs a `gh` login or a `GH_TOKEN`. The command exits 0 when the attestation is valid for the file.

`tsk self-update` and the session start installer run this check before they extract an archive. `TSK_REQUIRE_ATTESTATION=1` stops an install that cannot verify. See `docs/user-guide/installation.md`.

Releases are immutable: the assets and the tag of a published release cannot be changed.
