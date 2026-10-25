# Security Policy

Photon is currently in active development and has not yet had a stable release. We appreciate responsible reports of potential security vulnerabilities.

## Supported Versions

Photon does not currently have versioned releases or a formal security support schedule. Security issues affecting the current development branch (`main`) are reviewed and addressed on a best-effort basis. Older commits, unofficial builds, and forks are not guaranteed to receive fixes.

**Photon is pre-release software and should not be relied on for security-sensitive everyday browsing.**

## Reporting a Vulnerability

**Do not disclose suspected vulnerabilities in public issues, pull requests, or discussions.**

Please use GitHub's private vulnerability reporting:

- [Photon browser](https://github.com/PhotonBrowser/photon/security/advisories/new)
- [Photon Engine](https://github.com/PhotonBrowser/photon-engine/security/advisories/new)

These links require private vulnerability reporting to be enabled in the respective repository settings. If the feature is unavailable, contact the maintainers privately to arrange a secure reporting channel; do not post exploit details publicly.

Please include the affected component, operating system, commit if known, steps to reproduce, expected security impact, and a minimal proof of concept where appropriate. Remove credentials and personal information from any logs or attachments.

## Response and Disclosure

Photon is an early-stage project maintained on a best-effort basis. We aim to acknowledge actionable reports within seven days, but cannot guarantee an acknowledgement, fix, or disclosure timeline.

Maintainers will investigate reports, assess their impact, and coordinate fixes or mitigations where feasible. We may decline reports that cannot be reproduced or do not constitute a security vulnerability, and will try to explain that decision privately.

Please allow time for coordinated remediation before publicly disclosing vulnerability details. Do not access other people's data, disrupt services, or test systems without authorization.

## Upstream Dependencies

Photon incorporates a Ladybird-derived engine and other upstream projects. For vulnerabilities in shared upstream code, we may coordinate privately with the affected upstream maintainers while assessing Photon-specific exposure and mitigations.
