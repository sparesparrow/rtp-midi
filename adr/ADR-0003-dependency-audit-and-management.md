# ADR-0003: Dependency Audit and Management Policy

## Status

Accepted

## Context

The `rtp-midi` project is a Rust workspace with multiple crates, each with its own set of dependencies. The project relies on a variety of third-party crates for audio processing, networking, MIDI handling, concurrency, and platform integration. Managing dependencies is critical for security, maintainability, and performance. A `cargo tree --workspace` audit was performed to visualize and review all direct and transitive dependencies ([cargo tree documentation](https://doc.rust-lang.org/cargo/commands/cargo-tree.html)).

## Decision

- All dependencies must be reviewed for:
  - License compatibility
  - Security vulnerabilities
  - Maintenance status (actively maintained, not abandoned)
  - Suitability for real-time and cross-platform requirements
- Use the latest stable versions of dependencies unless a specific version is required for compatibility.
- Avoid duplicate versions of the same crate where possible to reduce binary size and build times.
- Use `cargo audit` and `cargo deny` in CI to enforce security and license checks.
- Document any exceptions or justifications for using crates with known issues or multiple versions.

## Consequences

**Positive:**
- Improved security and compliance through regular audits and CI enforcement.
- Reduced risk of supply chain attacks or unmaintained dependencies.
- Smaller binaries and faster builds by minimizing duplicate dependencies.
- Clear documentation of dependency choices and trade-offs for future maintainers.

**Negative:**
- May require additional effort to update or refactor code when dependencies are deprecated or have breaking changes.
- Some features may be delayed if suitable dependencies are not available or require significant integration work.

---

_This ADR follows the standard template and best practices for architecture decision records ([source](https://github.com/joelparkerhenderson/architecture-decision-record))._ 