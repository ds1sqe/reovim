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

For detailed development setup, see [docs/DEVELOPMENT.md](./docs/DEVELOPMENT.md).

## Testing

All changes must pass the test suite:

```bash
cargo test
```

When adding new functionality:
- Add tests for new code paths
- Ensure existing tests pass
- See [docs/TESTING.md](./docs/TESTING.md) for testing guidelines

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
