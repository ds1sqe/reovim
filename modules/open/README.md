# File opening command

Load the `open` native example once per Client through ordinary composition.
`command.open` v1 WRITE takes relative UTF-8 filename data and two Resource
slots: a WRITE `host.files.select` provider and a WRITE Source opening Model.
It selects a prepared shared file source, captures READ access, then invokes
`kernel.buffer.open`. Runtime installs independent state, History and backing
together. Cleanup drops the command's capture without closing the shared source.

The response is a Buffer descriptor under `command.open` v1: revision as a
little-endian u64 and Buffer/History/backing references. Buffer and History
slots grant ALL; the backing slot grants no operations. This is metadata, not
a Source observation or byte-length reply. Client composition supplies close
ownership; output grants
alone do not transfer it. Context attachment is a separate Client operation.

Opening reserves one of 16 in-flight/cleanup entries before selection. Failed
work cleanup remains owned by the command; a successful Buffer is still
returned. Later calls retry cleanup, and command unload reports failure or Busy
while ownership remains unresolved. No fallible work runs in Drop.

Nested unknown delivery first attempts ordinary request-result recovery. If
that cannot settle, the original recovery locator is preserved and the command
retains its stage and source/Model access. Resolve that locator
and pass the granted work as the single READ slot to `command.open.resume` v1
WRITE, with empty data. It resumes the stopped opening and returns the same
Buffer descriptor under the resume contract. It never selects again or creates
another Buffer for a recovered initialization. A completed job cannot be
resumed under a new request. Ordinary delivery retries use the same receipt.
The supplied work may be an outer command request; resume follows its Unknown
results to the retained stage. After adopting the final Buffer, close caller
work newest first, then call cleanup to release internal stage records. Those
records can stay Busy while outer requests still reference them.

Recovery follows at most 16 nested results per attempt and retains at most 16
internally consumed work records per opening. Exhaustion or unavailable
evidence stays Unknown. Internally adopted work is closed during cleanup;
work explicitly exposed to the caller remains available to that caller.

`command.open.cleanup` v1 WRITE with empty
data and no slots reconciles those results and retries work cleanup. Drain
before closing an exposed nested recovery result; drain does not close that
caller's work access or a returned Buffer. Missing recovery evidence remains
an explicit error, never permission to repeat an effect.

The `open_fault` example is a test witness. It retains actual selection and
initialization requests and rejects their first recovery read, exercising both
resume phases through the native ABI. It is not the ordinary command artifact.
Its `fail.md` fixture also sends an invalid initialization payload, allowing
the Client to verify a confirmed failed stage without a created Buffer.

The directory owns shared source installation until explicit idle release or
module unload. Host unload is Busy while a Buffer retains its association;
command unload is independent after its work drains. Closing one Buffer does
not close another Buffer's source. Retained state keeps its immutable reader
after source withdrawal. Selection can prepare a newer cut for later openings;
each Buffer retains the coherent cut acquired when its Kernel opening begins.

The producer uses Linux atomic clones or qualified read-lease copying, with
read-only backing. This command does not establish conditional saving.
