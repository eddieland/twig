# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| Latest  | :white_check_mark: |

## Reporting a Vulnerability

Please report security vulnerabilities via GitHub's private vulnerability reporting feature.

To report a vulnerability:

1. Go to the repository's Security tab
2. Click "Report a vulnerability"
3. Fill out the vulnerability report form with as much detail as possible

We take security seriously and will respond to vulnerability reports promptly. Please do not publicly disclose security vulnerabilities until we have had a chance to address them.

## Security Best Practices

When using Twig:

- Keep your installation up to date with the latest version
- Use secure credential storage (`.netrc` files with appropriate permissions)
- Be cautious when running plugins from untrusted sources
- Review plugin code before execution if possible

## Scope

This security policy applies to:

- The main Twig application and all its crates
- Official plugins and examples in this repository
- GitHub Actions workflows and automation

For security issues in third-party plugins or dependencies, please report them to the respective maintainers.
