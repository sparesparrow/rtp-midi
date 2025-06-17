# Tech Stack Audit

This document lists all major dependencies used in the rtp-midi workspace, including their versions, licenses, and a brief risk assessment. This audit helps ensure compliance, maintainability, and awareness of potential risks.

## Audit Process
- Dependencies were collected from all workspace and crate Cargo.toml files.
- License information is based on crate documentation and known standards for popular Rust crates.
- Risk assessment considers maintenance status, license compatibility, and known issues.

| Dependency         | Version    | License      | Risk Assessment |
|--------------------|------------|--------------|----------------|
| anyhow            | 1.0        | MIT/Apache-2 | Actively maintained, permissive license |
| log               | 0.4        | MIT/Apache-2 | Stable, widely used |
| env_logger        | 0.11       | MIT/Apache-2 | Stable, no known issues |
| ctrlc             | 3.4        | MIT/Apache-2 | Stable, no known issues |
| serde             | 1.0        | MIT/Apache-2 | Core Rust crate, actively maintained |
| serde_json        | 1.0        | MIT/Apache-2 | Core Rust crate, actively maintained |
| toml              | 0.8        | MIT/Apache-2 | Stable, no known issues |
| uuid              | 1.0        | MIT/Apache-2 | Stable, no known issues |
| bytes             | 1.0        | MIT/Apache-2 | Stable, no known issues |
| crossbeam-channel | 0.5        | MIT/Apache-2 | Stable, no known issues |
| num-traits        | 0.2        | MIT/Apache-2 | Stable, no known issues |
| url               | 2.5.0      | MIT/Apache-2 | Stable, no known issues |
| async-trait       | 0.1        | MIT/Apache-2 | Stable, no known issues |
| once_cell         | 1.19       | MIT/Apache-2 | Stable, no known issues |
| rand              | 0.8        | MIT/Apache-2 | Stable, no known issues |
| tokio             | 1          | MIT          | Core async runtime, actively maintained |
| tokio-tungstenite | 0.27.0     | MIT/Apache-2 | Stable, no known issues |
| futures-util      | 0.3        | MIT/Apache-2 | Stable, no known issues |
| reqwest           | 0.12.20    | MIT/Apache-2 | Stable, widely used |
| ddp-rs            | 1.0.0      | MIT          | Niche, but maintained |
| cpal              | 0.16.0     | Apache-2.0   | Stable, no known issues |
| rustfft           | 6.1        | MIT/Apache-2 | Stable, no known issues |
| midir             | 0.10.0     | MIT/Apache-2 | Stable, no known issues |
| opus              | 0.3.0      | BSD-3-Clause | Stable, no known issues |
| webrtc            | 0.9        | MIT/Apache-2 | Maintained, but evolving |
| libc              | 0.2        | MIT/Apache-2 | Core Rust crate, stable |
| jni               | 0.21.1     | MIT/Apache-2 | Stable, no known issues |
| android_logger    | 0.11.0     | MIT/Apache-2 | Stable, no known issues |
| rsbinder          | 0.4.0      | MIT/Apache-2 | Maintained, Android-specific |
| rsbinder-aidl     | 0.4.0      | MIT/Apache-2 | Maintained, Android-specific |
| mockito           | 1.2.0      | MIT/Apache-2 | Stable, test-only |
| tempfile          | 3.10       | MIT/Apache-2 | Stable, test-only |
| midi-types        | 0.2.1      | MIT/Apache-2 | Stable, no known issues |
| clap              | 4.5.4      | MIT/Apache-2 | Stable, no known issues |
| openssl           | 0.10.64    | Apache-2.0   | Stable, security-sensitive, keep updated |

## Summary
- **No unmaintained or problematically licensed dependencies were found.**
- All major dependencies use permissive licenses (MIT, Apache-2.0, BSD-3-Clause).
- All crates are actively maintained or stable, with no known critical issues at the time of audit.
- Security-sensitive crates (e.g., openssl) should be kept up to date.

_This audit should be repeated periodically and whenever new dependencies are added._ 