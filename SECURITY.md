# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✓ Current |

## Reporting a vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Use [GitHub's private vulnerability reporting](https://github.com/kent-tokyo/masstrust/security/advisories/new) to submit a report. You will receive a response within 7 days.

Please include:
- A description of the vulnerability and its potential impact
- Steps to reproduce
- Affected version(s)

## Scope

`masstrust` is a **local CLI tool and Rust library** for post-hoc annotation trust decisions. It reads CSV/Parquet files from disk and writes CSV/JSON output. It does not:

- Run as a server or accept network connections
- Execute code from input files
- Manage user credentials or secrets
- Perform privilege escalation

**In scope:** memory safety issues, path traversal in file I/O, malformed-input panics in the library.

**Out of scope:** denial-of-service via large input files (expected use case), issues in optional heavy dependencies (`polars`, `plotters`) beyond what can be fixed in `masstrust` itself.

## Disclosure

Once a fix is available, we will:
1. Publish a patched release
2. Open a public GitHub Security Advisory
3. Note the fix in `CHANGELOG.md`
