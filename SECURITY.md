# Security Policy

## Supported Versions

| Version          | Supported |
| ---------------- | --------- |
| Prod. Beta 5.X.X | ✔️        |

## Supported Packages

| Packages                                                    | Supported |
| ----------------------------------------------------------- | --------- |
| [AUR `zoi`](https://aur.archlinux.org/packages/zoi)         | ✔️        |
| [AUR `zoi-bin`](https://aur.archlinux.org/packages/zoi-bin) | ✔️        |
| [Homebrew](https://github.com/Zillowe/homebrew-tap)         | ✔️        |
| [Scoop](https://github.com/Zillowe/scoop)                   | ✔️        |
| [Crates.io `zoi-rs`](https://crates.io/crates/zoi-rs)       | ❔        |
| NPM `@zillowe/zoi`                                          | ❌        |

Crates.io package can be out-of-date sometimes.

The NPM package just runs the installer script.

Meanings:

- ✔️ Fully supported
- ❔ Could be out-of-date
- ❌ Not supported

## Security Updates

We take security seriously. Security updates are released as soon as possible after a vulnerability is discovered and verified.

Always make sure you have the latest Zoi version, to get the latest Zoi version install/update it from supported packages or by running this command:

```sh
zoi upgrade
```

## Reporting a Vulnerability

If you discover a security vulnerability, please follow these steps:

1. **DO NOT** disclose the vulnerability publicly.
2. Send a detailed report to: [GitLab Issues\*](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/issues/new), [GitLab Vulnerability](https://gitlab.com/Zillowe/Zillwen/Zusty/Zoi/-/security/vulnerabilities/new), email: [contact@zillowe.qzz.io](mailto:contact@zillowe.qzz.io) or [@ZilloweZ:matrix.org](https://matrix.to/#/@ZilloweZ:matrix.org).
3. Include in your report:
   - A description of the vulnerability
   - Steps to reproduce the issue
   - Potential impact
4. If you're creating an issue make sure you turn on confidentiality.
