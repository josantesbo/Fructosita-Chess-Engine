# Security Policy

## Supported version

Security fixes are applied to the latest public Fructosita release. Historical tags, including `v1.3.4`, are preserved for reproducibility and may not receive backports.

## Reporting a vulnerability

Please do **not** publish exploit details, credentials, private files, or other sensitive information in a public issue.

Preferred reporting method:

1. Use GitHub's **Private vulnerability reporting / Security Advisories** for this repository when available.
2. If private reporting is unavailable, open a public issue containing only a minimal, non-sensitive description and ask the maintainer for a private follow-up channel.

Include, when possible:

- affected version and operating system;
- exact input or command needed to reproduce the problem, with sensitive material removed;
- expected and observed behavior;
- whether the issue can cause arbitrary file access, code execution, denial of service, or data exposure.

## Safe use

Fructosita is a local UCI chess engine. Treat downloaded executables, opening books, EPD/PGN files, and engine configuration files as untrusted unless you know their origin.

For official releases:

- download assets only from the canonical repository;
- verify SHA-256 checksums when integrity matters;
- do not run third-party binaries merely because they use the Fructosita name;
- run the engine as a normal user rather than with administrator privileges;
- keep personal credentials, SSH keys, tokens, browser profiles, and unrelated private files outside experimental workspaces.
