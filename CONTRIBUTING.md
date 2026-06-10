# Contributing to Reovim

## Code Standards

### Zero-Warning Policy

This project enforces a **zero-warning policy**. Before submitting code:

1. Run `cargo build` - must produce zero warnings
2. Run `cargo clippy` - must produce zero warnings
3. Run `cargo fmt` - code must be formatted

No warnings are acceptable. This is non-negotiable.

### Code Quality

- Follow existing code patterns
- Keep functions focused and small
- Prefer clarity over cleverness

All architecture, ABI, and conformance rules live in the normative
specification at [`Documentation/`](./Documentation/README.md) — read
it before changing any boundary-crossing surface.

## Testing

All changes must pass the test suite:

```bash
cargo test
```

When adding new functionality:
- Add tests for new code paths
- Ensure existing tests pass
- Spec-governed surfaces additionally require the conformance fixtures
  listed in [`Documentation/09-Conformance/01-Rule-Matrix.md`](./Documentation/09-Conformance/01-Rule-Matrix.md)

## Development Workflow

1. Fork and clone the repository
2. Create a feature branch from `develop`
3. Make your changes
4. Run the full check suite:
   ```bash
   cargo build && cargo clippy && cargo test && cargo fmt --check
   ```
5. Submit a pull request to `develop`

## Pull Request Checklist

Before submitting:

- [ ] Code compiles with zero warnings (`cargo build`)
- [ ] Clippy passes with zero warnings (`cargo clippy`)
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] New code has appropriate tests
- [ ] Commit messages are clear and descriptive

## Commit Messages

Use clear, descriptive commit messages:

```
Add cursor word-forward motion

Implement w command for word-forward navigation.
Handles word boundaries and line ends.
```

## Contributor License Agreement

By contributing to this project, you agree to the following terms:

1. **Grant of Rights**: You grant ds1sqe (the project maintainer) a perpetual, worldwide, non-exclusive, royalty-free, irrevocable license to use, reproduce, modify, prepare derivative works of, publicly display, publicly perform, sublicense, and distribute your contributions and any derivative works.

2. **Dual Licensing**: You understand and agree that your contributions may be used under the AGPL-3.0 license for the open source version, and may also be included in commercially licensed versions of the software.

3. **Original Work**: You represent that your contributions are your original work and that you have the right to grant the above license.

4. **No Obligation**: The project maintainer is under no obligation to use or incorporate your contributions.

This CLA ensures that the project can maintain flexibility in licensing while keeping the core open source.
