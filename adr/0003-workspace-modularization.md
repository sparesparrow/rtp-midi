# ADR-0003: Workspace Modularization for Modular, Event-Driven Architecture

## Status
Proposed

## Context
The `rtp-midi` project has grown to encompass multiple domains: MIDI networking, audio analysis, LED output, platform integration, and more. The current structure centralizes most logic in `rtp_midi_lib`, making it difficult to maintain, test, and extend. The new architecture (see ADR-0001) calls for strict modularity, event-driven design, and clear boundaries between domains.

## Decision
We will modularize the workspace into separate crates, each responsible for a major domain. This will:
- Improve maintainability and testability
- Enable parallel development and clearer ownership
- Enforce boundaries and reduce coupling
- Align with Rust workspace best practices ([Rust Book](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html), [Medium Guide](https://medium.com/@aleksej.gudkov/rust-workspace-example-a-guide-to-managing-multi-crate-projects-82d318409260))

### New Workspace Structure
```
rtp-midi/
  core/           # Event bus, packet processor, recovery journal, mapping
  network/        # MIDI network I/O, signaling, network interface
  audio/          # Audio input, analysis, device, codec
  output/         # WLED/LED output, DDP, light mapping
  platform/       # FFI, JNI, platform bridges (Qt, Android, CLI, Web)
  utils/          # Shared utilities
  ...
```
Each will be a Rust crate, added to `[workspace.members]` in the root `Cargo.toml`.

### Migration Steps
1. Create new crates with `cargo new --lib <crate>` for each module.
2. Move relevant files from `rtp_midi_lib/src/` into the new crates.
3. Update the workspace `Cargo.toml` and each crate's `Cargo.toml` for dependencies.
4. Refactor `rtp_midi_lib` to orchestrate the new modules, becoming a thin integration layer.
5. Update documentation and diagrams to reflect the new structure.
6. Commit and push after each significant step.

## Consequences
- Code will be more modular, testable, and maintainable.
- Builds and CI will be faster and more reliable.
- Each domain can evolve independently, with clear interfaces.
- Some short-term breakage and refactoring will be required, but long-term velocity and quality will improve.

---
**Reviewed by:** [pending]
**Date:** [pending] 