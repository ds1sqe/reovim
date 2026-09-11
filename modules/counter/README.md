# Counter module

An Interface-only implementation with an inert factory and driven lifecycle.
Activation publishes `modules/counter/value`; unload removes it.

| `external.counter` v1 | Payload | Result |
| --- | --- | --- |
| Read | Empty | Current u64, little-endian |
| Write | u64, little-endian | Add the amount and return the new value |

Invalid input and overflow preserve the value. Each factory construction owns
fresh state. Rustdoc shows the public factory/provider usage.

The [server walkthrough](../../apps/server/README.md) exercises live activation
and unload through ordinary Resource requests.
