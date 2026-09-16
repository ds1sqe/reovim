# Caller transport

`protocol::Frame` and `Reply` encode the common caller records. `Endpoint`
binds a host-issued credential to an Interface Caller; sockets reconnect to
that retained owner. The [caller protocol](../../Documentation/Design/06.5-caller-protocol.md)
owns exact fields, bounds and error/outcome distinctions.

The local transport sends UTF-8 tab-separated frames terminated by a newline,
bounded to 64 KiB including the newline. Call input contains a common operation
code, opened access token, local ioctl command, optional expected state and
canonical bytes/references. A bound declaration fixes exec/ioctl value types.

Reserve before transmission. An exact repeated Call joins original work;
changed content rejects Conflict. After a lost reply, query Result on the same
Caller and receipt. UNKNOWN can be reconciled without another submission.
Construct returned access owners before Adopt, then Release the receipt.
Transport loss neither proves rejection nor authorizes another effect.

Opening, credential issuance and authentication remain host-owned. Keep the
opening secret and returned caller token private and retain them before sending
work. Endpoint token equality only selects already authenticated authority;
it does not establish transport confidentiality or create a new Caller.

The SDK's native integration suite exercises framed socket disconnect/recovery
through Endpoint, Runtime and an independently compiled C provider. Server/CLI
assembly and persistent client checkpoints are still migrating. Retired
command-name/Contract/version frames and checkpoint records are not accepted
by the new operation codec; there is no translation path.
