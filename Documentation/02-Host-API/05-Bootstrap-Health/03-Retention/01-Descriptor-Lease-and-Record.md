# OutputHealth Descriptor, Lease, and Record Result

[Retention index](../03-Retention-Lease-and-Snapshot.md)

## Public descriptor values

The sibling Kernel Interface `output_health` module owns these state-facing
public values:

- `OutputHealthLimits`;
- `OutputHealthSnapshotLimits`;
- `OutputHealthDescriptor`;
- `OutputHealthLeaseScope`;
- `OutputHealthProposalScope`;
- `OutputHealthAcceptance`;
- `OutputHealthRecordOutcome`;
- `OutputHealthError`; and
- the `OutputHealthLease` trait.

OutputHealth has no independent lifecycle identity. Its descriptor contains
one `ContractSelection`, the exact source Event Stream `LifecycleIdentity`,
that Stream's Lifecycle Owner, and its limits. Negotiation distinguishes an
unsupported contract identity from an incompatible version.

`OutputHealthSnapshotLimits` groups nonzero record and charged-byte maxima so
the primary limits constructor remains small and typed. `OutputHealthLimits`
binds every charge to the Stream instance and declares:

- nonzero bounded work for one record operation;
- exact-zero temporary record-operation bytes;
- nonzero fixed partition-state resident bytes;
- one nonzero fixed charge for each complete anomaly slot;
- nonzero retained-anomaly count;
- checked derived anomaly-capacity and total partition resident bytes;
- nonzero caller-owned snapshot record and charged-byte maxima; and
- nonzero bounded import attempts without granting import authority.

Snapshot maxima do not exceed retained capacity. The snapshot byte maximum
fits at least one fixed anomaly charge. Checked multiplication and addition
fail before limits construction. Fixed partition bytes own counters,
synchronization, and non-record state. Each anomaly charge owns its complete
proposal scalars, health sequence, slot/index, and reserved capacity.

These are authority-declared portable accounting values, not Rust layout,
serialization length, allocator bookkeeping, or RSS. A concrete mechanism
must separately prove that its carriers fit every declared charge.

## Operation classification

Descriptor inspection is pure. Recording and snapshot are exactly:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Neither operation waits, allocates output, invokes a callback, performs I/O,
or enters Event admission. Contention returns a typed outcome.

## Generation-bound lease scope

`OutputHealthLeaseScope::new` derives one DiagnosticOutput lifecycle and its
source Stream from a `DiagnosticOutputDescriptor`, then validates that source
against the health descriptor. It also retains the health Stream and Lifecycle
Owner. A caller cannot supply duplicate output/source identity fields.

The scope classifies a proposal in this order:

1. another output or source Stream incarnation is `Invalid`;
2. the same incarnation with another generation is `Stale`; and
3. exact output and source generations are `Current`.

`OutputHealthProposalScope` contains exactly those three values. Scope
construction and classification copy scalars only. They neither prove that
Assembly granted a lease nor observe live state.

## Write-only port and result

`OutputHealthLease` exposes only:

```text
descriptor(&self) -> &OutputHealthDescriptor
scope(&self) -> OutputHealthLeaseScope
record(&self, OutputHealthProposal) -> OutputHealthRecordOutcome
```

It cannot snapshot, import, render, admit an Event, invoke a callback, or reach
an Event Stream object. An adapter cannot mutate counters or select a health
sequence directly.

`OutputHealthAcceptance` is exactly `CounterOnly` or
`AnomalyRetained(OutputHealthSequence)`. `OutputHealthRecordOutcome` is
exactly:

- `Accepted(OutputHealthAcceptance)`;
- `Pressure`;
- `Invalid`;
- `Stale`;
- `Closed`; or
- `SequenceExhausted`.

Possessing one constructible result value is not historical proof. Only a
value returned by a live lease at the atomic record cut claims that operation
outcome.

## Atomic record relation

The record operation has this observable precedence:

1. classify the proposal against the immutable lease scope;
2. attempt non-waiting state authority, returning `Pressure` on contention;
3. revalidate the current Stream generation, returning `Stale` on replacement;
4. reject Closing or Closed state with `Closed`;
5. apply a counter-only proposal and return `Accepted(CounterOnly)`; or
6. for an anomaly, reject sequence exhaustion, evict complete oldest records
   until one fixed slot fits, then atomically update counters, assign sequence,
   retain the complete record, and return its sequence.

Invalid, stale, pressure, closed, and sequence-exhaustion results use their
fixed non-waiting result-counter path while health remains live. After terminal
Closed, an attempt returns `Closed` and claims no terminal-state mutation.
Counter-only recording remains available after anomaly sequence exhaustion.

After Closing, a retired-generation lease remains usable only for work already
accepted before the Closing cut, including failed-owned work, until terminality
or containing-process reap. It grants no new DiagnosticOutput submission
authority. The live owner and adapter must prove this history; it is not
inferable from a proposal or scope value.

## Error and trait contract

`OutputHealthError` distinguishes Foundation bound/arithmetic failure, limits
owner or Stream mismatch, unsupported/incompatible contract, zero/exhausted
sequence, counter-only anomaly, source mismatch, invalid retained range,
snapshot bound, and inconsistent snapshot metadata or records.

Inputs validate in signature order, then identity/owner relations, then
semantic joins. Display text is stable semantic wording and the error
implements `Error`.

All immutable values in this contract family derive
`Clone + Copy + Debug + PartialEq + Eq`. The trait has no concrete mechanism or
automatic thread-safety claim.
