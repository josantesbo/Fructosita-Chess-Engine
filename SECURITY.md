# Security Policy

## Supported version

Security fixes go into the latest public Fructosita release. Older tags stay available for reproducibility and may not receive fixes.

## Reporting a vulnerability

Please do **not** publish exploit details, credentials, private files or other sensitive information in a public issue.

1. Use GitHub's **Private vulnerability reporting / Security Advisories** for this repository when it is available.
2. Otherwise, open a public issue with a short, non-sensitive description and ask for a private channel.

When possible, include:

- affected version and operating system;
- the exact input or command that reproduces the problem, with sensitive material removed;
- expected and observed behaviour;
- whether the issue can lead to file access, code execution, denial of service or data exposure.

## Safe use

Fructosita is a local UCI chess engine. Treat downloaded executables, opening books, EPD and PGN files and engine configuration files as untrusted unless you know where they come from.

For official releases:

- download assets only from this repository's Releases page;
- check the SHA-256 checksums when integrity matters;
- do not run third-party binaries just because they use the Fructosita name;
- run the engine as a normal user, not as administrator.
