# Security policy

## Supported versions

terminfokit is pre-release software. Only the `main` branch is currently
supported; there is no supported packaged release yet. Older snapshots are not
maintained.

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability.

Use
[GitHub private vulnerability reporting](https://github.com/P4suta/terminfokit/security/advisories/new)
to share a description, affected inputs or platforms, impact, and a minimal
reproducer when one is available. Reports may cover the Rust API, compatible
CLI frontends, binary/source parsers, database search and installation, escape
expansion, or CI and supply-chain configuration.

The maintainer will aim to acknowledge a report within seven days, validate its
scope, and coordinate disclosure and remediation through the private advisory.
Please allow a reasonable remediation window before publishing details.

## Hardening and disclosure

The public repository uses dependency alerts and updates, secret scanning with
push protection, CodeQL advanced setup, dependency review, pinned GitHub
Actions, and protected default-branch rules. Valid fixes are credited in the
advisory unless the reporter prefers to remain anonymous.
