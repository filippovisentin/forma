# Security Policy

## Supported Versions

Forma is a personal project with a single active release line. Security fixes are applied to the latest release only.

| Version | Supported          |
| ------- | ------------------ |
| Latest  | :white_check_mark: |
| Older   | :x:                |

## Reporting a Vulnerability

Please report security vulnerabilities **privately** by emailing:

**filippovise2003@gmail.com**

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested fixes

We will:
- Acknowledge receipt within 48 hours
- Provide a timeline for assessment and fix
- Credit you in the release notes (unless you prefer anonymity)

## Disclosure Policy

- Vulnerabilities will be fixed in the next patch release
- Public disclosure occurs after a fix is available
- No bug bounty program exists (personal project)

## Scope

This policy covers:
- The Forma application (`forma.exe`, `forma-cli.exe`)
- All crates in this workspace
- Build and release infrastructure (GitHub Actions)

Out of scope:
- Third-party dependencies (report upstream)
- Operating system / driver vulnerabilities
- Social engineering / physical access attacks