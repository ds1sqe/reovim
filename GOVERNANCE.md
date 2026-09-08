# Governance

## Decision model

Reovim uses a Benevolent Dictator For Life model.

**BDFL:** [@ds1sqe](https://github.com/ds1sqe)

The BDFL has final authority over product scope, architecture, public contracts,
feature admission, dependencies, release claims, and project policy.

## Authority separation

- [Documentation](Documentation/README.md) owns stable goals, terminology,
  architecture, contracts, conformance, migration, and release acceptance.
- Durable plans own dependency order and implementation phases.
- tmp/current.md owns live Mission, Progress and Focus; completed.md in the
  plan directory records tasks moved after their successful commits.
- Flight logs own decisions, chronology and evidence pointers.
- GitHub issues and milestones own public work tracking.
- Code, tests, and retained evidence establish implementation conformance.

Tracker state, implementation convenience, or a historical subsystem cannot
override a normative contract. Review contract changes with their behavior,
failure and verification requirements; the old exact-proof registry is retired.

## Product priorities

1. source fidelity and prevention of silent corruption or loss;
2. atomic state authority and deterministic conflict behavior;
3. explicit Effect Certainty, recovery, cancellation, and shutdown;
4. bounded resources, isolation, and visible pressure;
5. actionable diagnostics and reproducible evidence;
6. measured reaction latency, memory, I/O, and extension cost; and
7. feature breadth.

Reovim favors a smaller evidenced surface over a feature-rich but unreliable
one. Mission-critical language is an engineering discipline, not an unnamed
formal certification claim.

## Contributions and dependencies

Contributions are welcome and remain subject to BDFL review. Public behavior
uses public contracts with no first-party shortcut. Third-party dependencies,
extensions, and adapters are accepted selectively with explicit trust,
compatibility, maintenance, license, target, unsafe/native, runtime,
performance, and evidence review.

## License

Reovim is AGPL-3.0 with owner exception. The BDFL retains the right to use,
modify, and distribute the code under alternative licenses. Contributions are
made under AGPL-3.0 and the terms in [CONTRIBUTING](CONTRIBUTING.md).
