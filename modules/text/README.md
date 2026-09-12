# Native Text interpretation

Load `text` as a client-owned Kernel module (`--client-module` or that client's
module loader). It captures a granted source Buffer; attaching creates no new
Buffer or History. Each attachment is one immutable interpretation lifetime:
its Resource identity also identifies its space. Another attachment is foreign
even when it interprets the same Buffer. A shared server-wide module binding
with a different cleanup owner is not this profile.

Positions count Unicode scalars plus one opaque unit per undecodable source
byte. NUL, CRLF, combining marks and invalid bytes keep their original meaning;
grapheme motion and rendering belong above this API. Inserted text must be valid
UTF-8. Selecting an opaque unit can remove it; other opaque units survive.

| Contract v1 | Input | Output |
| --- | --- | --- |
| text.attach / text.attach.read | Empty; one Buffer ALL/READ reference | Owned interpreted access ALL/READ |
| text.observe | Empty READ on access | revision/node/byte-length u64s; [access, space, Buffer, History, snapshot, watch], all READ |
| text.window | READ snapshot: start/count u64s | EOF:u8, start/end u64s, then five-byte units |
| text.replace | WRITE access: start/end u64s + UTF-8; one semantic snapshot READ reference | Actual Buffer observation with source byte length |
| text.next | READ watch: empty waits, `[0]` checks | Existing Buffer event envelope, qualified by this Text observation |

All integers are little endian. Unit encoding is tag:u8 + value:u32: tag 0 is a
Unicode scalar, tag 1 an opaque byte. Window count is at most 1024; EOF can return
fewer units. A start past EOF rejects. Byte and Text coordinates are distinct.
The Rust API creates observation metadata only from actual replies and qualifies
change delivery with the interpretation/space/Buffer identities.

Snapshot and watch are independent owned Resources. Close a snapshot or watch
to release it; close the interpreted access to detach. Detach fences new calls,
returns Busy while admitted work drains, then ends its watches with conservative
loss indication. Snapshots remain readable after detach. Failed cleanup retains
its owner for retry. Native code stays pinned until all its owners drain.

Observe uses a coherent Buffer snapshot/watch and prepares semantic wrappers.
Partial setup is cleaned before returning failure; failed underlying cleanup
remains owned by the interpreted access. The module permits 32 active operations
and up to 32 live snapshots/watches per access, within Kernel resource bounds.
Slow Buffer readers report Gap; renew observe and release the old watch.

The reader scans 4096-byte source chunks and carries incomplete UTF-8 prefixes.
Each snapshot caches decoded unit/byte positions, initially every 1024 units,
with at most 4096 marks. A full index becomes coarser; allocation failure skips
optional cache growth. Repeated reads start at a known preceding boundary.
Successful Text splices can seed the next snapshot's index using their exact
range/replacement correspondence, only when its Buffer state matches. Unrelated
edits start a fresh index; old snapshots retain their own positions.

First access to an unindexed prefix remains linear. There is no hidden whole
document materialization. Sparse residency, full performance qualification,
Resource-valued History metadata and Markdown preview remain further work.
