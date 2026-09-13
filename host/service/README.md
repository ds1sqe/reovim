# Local command transport

Experimental loopback TCP framing for the server/CLI executable path. Each
socket carries one UTF-8 request and a final response, bounded to 64 KiB
including newline. A pending operation may emit `pending` lines before its
final response; these keep a waiting connection observable without executing
the request again. Fields are tab-separated; payload bytes and recovery strings use hex.

```text
v1  open
v1  open     opening-secret
v1  recover  opening-secret
ok  client-token  boot-receipt-id
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
Its response waits for configured client startup. That work occupies one
ordinary retained receipt whose input is `boot`; acknowledge it with `release`
after adopting readiness. Failure returns
`error boot TOKEN RECEIPT rejected|failed|unknown CODE ID RECOVERY` with tab
separators, retaining partial startup ownership for inspection and cleanup.
Its opaque token selects that same logical client across sockets; another token
has different IDs and grants. The reference Linux host uses its random source
and bounds live clients to 16. Selecting a token does not start a new Kernel
connection epoch. This profile provides no persistence across server restart.

Resumable callers generate a random 128-bit lowercase hex `opening-secret`
and persist it before sending `open SECRET`. Treat it as a credential. While
the Client lives, that secret selects its original startup receipt. After any
uncertain transmission use only `recover SECRET`, which never creates a Client.
Missing evidence, including after server restart or teardown, is an error,
not permission to open again. Save the returned token and startup result before
releasing the boot receipt; its result is unavailable after release. Legacy
`open` without a secret cannot recover a lost handshake response.

Reserve an identity before transmitting operation fields. The client retains
up to 64 receipts; released identities are never reused. The first `call` records
its exact UTF-8 operation fields before admission. An identical retry joins the
same execution/result; changed fields reject without changing the original.

```text
resolve path
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
`resolve` returns `ok @ID` with tab separation. It performs passive lookup;
the CLI uses that exact identity when searching command directories, so a
subsequent path replacement cannot redirect the selected invocation.

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
`v1 TOKEN disconnect` fences owned subscriptions and attempts private cleanup.
Pending receipts and live Buffer/Session state keep the client recoverable;
adopt results and explicitly close state before retrying a Busy teardown.
Successful teardown is idempotent within the bounded recently closed token set.

## Rust receipt custody

`request::Request` is a portable caller-owned reservation/outcome state used by
the CLI. Construct it after reservation with the exact client, request identity
and operation fields. `send` marks transmission before returning the immutable
frame and rejects a second call. After uncertain I/O, `recover` requests the
original result. `record` retains a complete outcome; it rejects malformed or
contradictory replies without authorizing another execution.

Validate the module result and adopt its Resource owners before `adopt`.
Unknown cannot be adopted as ordinary failure. `release` then supplies retryable
receipt cleanup, and `released` records confirmed success while preserving the
adopted result. Transport errors do not consume this owner. The CLI presents
unresolved receipt identities for explicit recovery by its user.

This value supplies neither I/O nor a native Connection implementation. It
does not own returned Resources, perform startup or implement journal restart
restoration. A hosted TUI still needs its own transport, persistence and
semantic consumer; this profile is not the complete Rust remote SDK.
