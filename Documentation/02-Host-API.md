# Host API

> **Normative contract index.** The linked chapters jointly define the Host
> Capability boundary. This index is navigation, not a second authority.

## Reading order

1. [Boundary](02-Host-API/01-Boundary-and-Capability-Admission.md)
   defines ownership, dependency direction, the initial closed scope, and the
   consumer-driven admission rule.
2. [Clock](02-Host-API/02-Monotonic-Clock.md) defines opaque instant,
   operation, lifecycle, and adapter semantics.
3. [Output](02-Host-API/03-Diagnostic-Output.md) defines canonical-byte
   submission and destination evidence.
4. [Shutdown values](02-Host-API/04-Diagnostic-Shutdown-Values.md) defines
   shutdown entries, candidates, latching, and ownership evidence.
5. [Bootstrap health](02-Host-API/05-Bootstrap-Health.md) defines the one
   non-recursive failure state and OutputHealth partition.
6. [Lifecycle](02-Host-API/06-Lifecycle-Adapters-and-Conformance.md)
   defines readiness, shutdown, growth, substitution, and evidence.

The [Rule Matrix](06-Conformance/02-Rule-Matrix.md) remains the stable
obligation router. Present-tense requirements define the v0.16 target and do
not assert implementation or qualification.
