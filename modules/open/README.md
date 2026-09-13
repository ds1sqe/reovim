# File opening command

Load the `open` native example once per Client through ordinary composition.
`command.open` v1 WRITE takes relative UTF-8 filename data and two Resource
slots: a WRITE `host.files.capture` provider and a WRITE Source opening Model.
It captures current file contents, opens independent Source state, and closes
the original capture. Source state retains its own immutable read view.

The response is the normal Buffer observation envelope, under `command.open`
v1, with ALL Buffer and History slots. Its semantic descriptor is byte length
as little-endian u64. Client composition supplies close ownership; output grants
alone do not transfer it. Context attachment is a separate Client operation.

Opening reserves one of 16 in-flight/cleanup entries before capture. Failed
capture cleanup remains owned by the command; a successful Buffer is still
returned. Later calls retry cleanup, and module unload reports failure or Busy
while ownership remains unresolved. No fallible work runs in Drop.

Nested unknown delivery first attempts ordinary request-result recovery. If
that cannot settle, the original recovery locator is preserved and the command
retains its capture bookkeeping. `command.open.cleanup` v1 WRITE with empty
data and no slots reconciles those results and retries capture cleanup. Drain
before closing an exposed nested recovery result; drain does not close that
caller's work access or a returned Buffer. Missing recovery evidence remains
an explicit error, never permission to repeat an effect.

The current producer uses Linux atomic file clones. This command does not
establish writable backing association, save semantics or non-clone storage.
