# Conformance Requirements and Evidence

## Purpose and authority

Normative Documentation defines stable obligations and evidence requirements.
Plans define execution order. Flight logs record chronological progress.
GitHub records public work state. Benchmark and test artifacts retain results.
None of those temporal facts belongs in the normative contract merely because
it is current when the contract is written.

A document, type name, passing narrow test, or historical implementation is
not by itself conformance evidence.

A test compares semantic values, keys, sets, ordered relations, or outcomes.
It does not use a literal checkout digest, serialized byte length, or aggregate
cardinality as a proxy for meaning. Published algorithm vectors and exact
protocol, capacity, accounting, or safety bounds remain valid when their owner
is named. Mechanically generated cache identities are performance inputs, not
test expectations or evidence.

Normative target language does not imply current implementation. Current
physical dependency truth comes from build metadata and generated or retained
evidence; implementation status comes from code plus matching conformance
results; execution chronology remains in plans and logs. Two documents cannot
grant conflicting authority over the same question.


## Chapter map

This file owns the Conformance authority boundary and routes to the focused
normative chapters without restating them:

1. [Evidence Taxonomy and Authority](03-Evidence-Taxonomy-and-Authority.md)
2. [Source Classification and Exact Checker](04-Source-Classification-and-Exact-Checker.md)
3. [Public Reduction and Qualification](05-Public-Reduction-and-Qualification.md)
4. [Normative Rule Matrix](02-Rule-Matrix.md)
