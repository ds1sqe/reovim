# Local command transport

Experimental loopback TCP framing for the server/CLI executable path. Each
socket carries one UTF-8 request and a final response, bounded to 64 KiB
including newline. A pending operation may emit `pending` lines before its
final response; these keep a waiting connection observable without executing
the request again. Fields are tab-separated; payload bytes and recovery strings use hex.

```text
v1  open
ok  client-token
v1  client-token  reserve
ok  request-id
v1  client-token  call     request-id  operation-fields...
v1  client-token  status   request-id
v1  client-token  result   request-id
v1  client-token  input    request-id
v1  client-token  release  request-id
v1  client-token  requests
```

`open` creates a retained client with a private namespace and native loader.
Its opaque token selects that same logical client across sockets; another token
has different IDs and grants. The reference Linux host uses its random source
and bounds live clients to 16. Selecting a token does not start a new Kernel
connection epoch. This profile provides no persistence across server restart.

Reserve an identity before transmitting operation fields. The client retains
up to 64 receipts; released identities are never reused. The first `call` records
its exact UTF-8 operation fields before admission. An identical retry joins the
same execution/result; changed fields reject without changing the original.

```text
ls     path
read   path-or-@id  contract  version  [hex [refs]]
write  path-or-@id  contract  version  [hex [refs]]
close  path-or-@id
ok     contract  version  hex  comma-separated-@ids
error  rejected|failed|unknown  code  optional-@id  optional-recovery-hex
error  control-error-detail
```

`ls` returns `ok` followed by `name=@id` fields. References are comma-separated
paths or caller-local `@ID` values; use `-` for empty hex data. Kernel validates
grants and contract slots. Root selection/aliases grant no private peer access.

`status` reports reserved, pending or finished. Reserved does not establish
that a delayed call can never arrive. `result` waits for or returns the original
response, including its same reference identities. It does not execute again.
`input` returns the original operation fields as hex. `requests` lists retained
IDs and states so an interrupted caller can find its receipts.

`release` rejects pending work, removes a completed delivery receipt or closes
an unused reservation against late calls. Releasing an old identity is
idempotent. It does not release separately returned Resource access. Missing
receipts do not prove that an effect never happened. Retrieve before releasing
when the result is unknown; never submit a fresh identity as automatic recovery.

The owner drives Kernel execution independently of up to 32 socket workers.
Waiting calls and incomplete frames leave other clients responsive. A failed socket
write removes only its observer; its receipt/execution stays with the client.
Explicit client teardown and native owned/subscription access remain unfinished.
