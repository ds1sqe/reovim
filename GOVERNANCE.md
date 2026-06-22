# Governance

## BDFL Model

Reovim follows a **Benevolent Dictator For Life (BDFL)** governance model.

**BDFL:** [@ds1sqe](https://github.com/ds1sqe)

The BDFL has final authority on all project decisions including:

- Architectural direction
- Feature acceptance
- Code standards
- Release timing
- Community guidelines

## Project Vision

Reovim aims to be the fastest-reaction text editor with a scalable, kernel-inspired architecture.

Priorities:

1. **Performance** - Minimal latency, instant response
2. **Modularity** - Kernel, drivers, modules separation
3. **Extensibility** - Rust, C, Haskell, Python modules
4. **Simplicity** - Unix philosophy

## Contributing

Contributions are welcome. All contributions are subject to BDFL approval.

Requirements:

- Associated GitHub issue
- Pass `./scripts/check.sh`
- Follow the documented architecture and testing standards in this repository
- Update CHANGELOG.md

## Code Standards

- Zero-warning policy (clippy pedantic)
- Layer boundaries enforced (kernel purity)
- Tests required for new functionality

## License

**AGPL-3.0** with owner exception.

The BDFL ([@ds1sqe](https://github.com/ds1sqe)) retains the right to use, modify, and distribute the code under alternative licenses.

All contributions are made under AGPL-3.0.
