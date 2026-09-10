# 07 — Worked examples

Rust examples using proposed helpers over
[Resource access](01.2-resource-access.md).
Composition supplies granted caller access; component API packages supply
optional codecs/helpers. Runtime drives progress. `root` is an authorized
namespace root; paths are composition-installed aliases.

Ordinary calls await results. The SDK owns [request recovery](01.3-requests.md)
across errors and dropped waits; it retires settled request records. Callers
close returned snapshots/access when finished. Unresolved effects stay owned
for reconciliation; never retry them as new work. These are design
walkthroughs, not execution or hardware qualification evidence.

The [module journey](07.1-module-journey.md) includes assembly, live providers,
Session/global state, Vim/Emacs and safe unload. In every example, Clients own
user input, views and rendering; Kernel receives semantic operations.

## Recursive Buffers, clipboard and settings

```rust
// Proposed wrappers; model and example_edit come from the selected Domain API.
let reference = tree.resolve_at(root, "buffers/original").await?;
let original = buffers.join(reference).await?;
let draft = buffers.open(original.state.buffer).await?;
// Draft state/History is independent; backing is the original Buffer.
let draft_view = model.attach(draft.state.buffer).await?;
let observed = model.observe(draft_view).await?;
let edit =
    Edit { observed: observed.snapshot.observed, value: example_edit };
let changed = model.edit(&edit).await?;
let published = publication.save(&changed.after).await?;
// Publication enters original's acceptance/History, then stops at that hop.
// Undoing draft cannot retract publication or save original's backing.
```

Each hop checks grants/preconditions/outcomes. Clipboard exposes typed format
offers with explicit replacement. A settings draft activates only at its
owner's publication acceptance. Log/History inspection records no edit per
read; recursive references cannot trigger recursive saves.

## Capture ownership

A capture imports a selected input into a Buffer through ordinary acceptance.
Its profile names batching, input/Buffer, start boundary, format, status,
bounds and known local loss.

```text
receive -> own/retain data -> accept import -> Buffer state + History
stop    -> fence admission/late installation -> report final accepted state
// Already committed imports stay included; cancelled work cannot appear later.
// Buffer close stops capture; input close leaves accepted data intact.
// Export retains its selected state independently of continued input/close.
// Transfer or retain bounded backing; receive scratch memory cannot own it.
```

Bound aggregate fan-out/export retention; slow exports cannot pin unlimited
data. Close drains/transfers export ownership or reports incomplete-shutdown.
Detach leaves continuing capture under an explicit owner, such as an
authorized Session. Rolling eviction needs a policy for data, History and
exports. Pause cannot promise to pause hardware/remote senders. An inspected
connection and Reovim's service transport keep separate contracts/grants even
when sharing a carrier.

## UDP to Buffer to file

```rust
// Proposed wrappers; buffers, captures, files and values use selected APIs.
let endpoint = tree.resolve_at(root, "connections/telemetry").await?;
let input = tree.resolve_at(endpoint, "input").await?;
let packets = buffers.create(&Datagrams::empty()).await?;
// StopAtBound is this capture profile's retention policy.
let capture =
    captures.start(input, packets.state.buffer, StopAtBound).await?;
// Create in the capture model; an endpoint need not offer a state snapshot.
// input/output/info share one endpoint owner; only explicit output sends.

let selected = buffers.describe(packets.state.buffer).await?;
let dump = files
    .export(selected.state, "files/capture.dat", DatagramRecords)
    .await?;
// Retain selected data; exclude later arrivals. Export never rebinds packets.
// DatagramRecords preserves message boundaries and declared metadata.
let stopped = captures.stop(capture).await?; // Final accepted Buffer state.
let editable_copy = buffers.open(packets.state.buffer).await?;
// This example edits a frozen independent copy and stops visibly at the bound.
```

UDP preserves datagrams without reliable delivery/order/deduplication. [RFC
768][udp]. Receive cursors are local evidence. Preserve declared sender,
observation time and clock identity; invent no missing metadata. Select one
receiver or explicit bounded fan-out. Equal payloads are distinct
observations; retrying the same accepted import must not duplicate it.

Dump exports one state, not all History/traffic. Raw concatenation is
explicitly lossy. Remote silence is not EOF; report known local loss without
inventing total remote loss. Stop, retention exhaustion, receive-storage
release and file failure must preserve accepted data and truthful file-effect
outcomes.

## GPIO: local desired state, explicit output

```rust
// Proposed GPIO/sample API helpers; request options come from those APIs.
let output_line = tree.resolve_at(root, "gpio/test-output").await?;
let input_line = tree.resolve_at(root, "gpio/test-input").await?;
// Capture uses this distinct input line.
let output = gpio.acquire_output(output_line, ActiveLow, Inactive).await?;
// Explicit acquisition sets initial configuration and retains its owner.
// Resolution/inspection alone never acquires hardware control.
let initial = LogicalInitial { active: false }; // From the Buffer chapter.
let desired_state = logical_buffers.create(&initial).await?;
let desired_view =
    logical_output.attach(desired_state.state.buffer).await?;
let observed = logical_output.observe(desired_view).await?;
let edit = Edit { observed: observed.snapshot.observed, value: Active };
let changed = logical_output.edit(&edit).await?;
let restored = history.undo(desired_state.history, &changed.after).await?;
// Edit and Undo changed only the local Buffer/History; no output write.
let effect = gpio.apply_selected(&output, &restored.after).await?;
// Capture the selected value and exact request/configuration lifetime.
// Driver acceptance, requested-value readback and physical feedback differ.

let samples = sample_buffers.create(&Samples::empty()).await?;
let input = gpio.acquire_input(input_line, BothEdges).await?;
let input_capture = captures.start(input, samples.state.buffer).await?;
let stopped = captures.stop(input_capture).await?;
let dump = files
    .export(stopped.state, "files/input-samples.dat", SampleRecords)
    .await?; // Input capture/export grants no output control.
```

The Linux v2 request owns exclusive line access. Closing the chip descriptor
leaves requests alive; releasing a request may change output/default state.
Edge detection requires input mode. [Line requests][gpio-request]. Readback
may report requested logical value rather than measured pad voltage.
[Readback][gpio-read]. Queued edges survive reconfiguration and may overflow;
sequence evidence exposes loss. [Edges][gpio-edge].

The adapter distinguishes chip offsets from header pins and retains direction,
active-low polarity, clock and configuration context. Old edges keep old
meaning. Fence request/configuration changes; serialize output writers.
Samples are not atomic revisions and versioned save is not automatically
available. Reject unsupported host guarantees; pulse/toggle are explicit
one-shot actions.

Test active-low output after local Undo, stale handoff after reconfiguration,
edge overflow and view close with a retained output owner. Release follows the
fixture's declared policy; crash promises no safe residual voltage. New
acquisition cannot replay an uncertain write or prove an attached device
acted.

## WebSocket: messages and reconnect

```rust
// Proposed WebSocket/message API helpers; Messages is the model input.
let connection = tree.resolve_at(root, "connections/chat").await?;
let inbox = buffers.create(&Messages::empty()).await?;
let capture = captures.start(connection, inbox.state.buffer).await?;
let draft = buffers.create(&Messages::from_text("hello")).await?;
let effect = websocket.send(connection, &draft.state).await?;
// Admission retains this payload and connection lifetime.
// Later draft edits do not change sending; Undo/save never sends again.
// Proof names local submission OR correlated application acknowledgement.

let replacement = websocket.reconnect(connection).await?;
// New lifetime: preserve Buffers and old uncertain outcomes; no blind replay.
// An old acknowledgement cannot settle a new send.
let selected = buffers.describe(inbox.state.buffer).await?;
let dump = files
    .export(selected.state, "files/messages.dat", MessageRecords)
    .await?; // Retained Buffers can export independently of the connection.
```

The adapter assembles bounded complete text/binary messages across
fragmentation while handling interleaved control frames. Sending Close forbids
later data frames. [RFC 6455][ws]. Reovim requires application acknowledgement
for application commitment; Ping/Pong alone supplies no such proof.

Oversized/invalid/incomplete data cannot become complete messages. Raw-frame
inspection needs its own model. Ping is no edit. Capture stop leaves shared
protocol handling alive. Normal Close is lifecycle, not necessarily Error; it
fences sends and performs bounded teardown while retaining queued/uncertain
outcomes. Pressure before admission leaves input unchanged; after handoff it
cannot prove rejection. Replay requires an explicit application
reconciliation/idempotency contract.

Test fragmentation around Ping, equal messages as distinct observations, edits
during send, lost completion, reconnect/old acknowledgements and export after
close. Buffers and required recovery evidence must survive.

## Shared failure checks

Exercise stale/denied/unsupported/malformed input, replacement, pressure,
cancellation, loss before/after admission, partial/unknown effects, failed
recovery and incomplete shutdown. Verify actual state/History, retained
exports, visible gaps/expiry and causal input-to-acceptance-to-presentation
evidence. Lost diagnostics never change certainty. Use deterministic faults,
then selected real adapters and [measured workloads](../performance.md).

[udp]: https://www.rfc-editor.org/rfc/rfc768.html
[gpio-request]:
  https://docs.kernel.org/userspace-api/gpio/gpio-v2-get-line-ioctl.html
[gpio-read]:
  https://docs.kernel.org/userspace-api/gpio/gpio-v2-line-get-values-ioctl.html
[gpio-edge]:
  https://docs.kernel.org/userspace-api/gpio/gpio-v2-line-event-read.html
[ws]: https://www.rfc-editor.org/rfc/rfc6455.html
