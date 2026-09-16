# Interface declarations and dynamic values

This chapter owns declaration identity, methods, value layouts and checked
binding. [Native dispatch](06.2-native-dispatch.md) owns callback/request records;
API packages own domain meanings. No Kernel enum contains Parse, ELF or NTFS.

## Identity and binding

An ApiId is a fixed 16-byte opaque key supplied by the API package. It
identifies one declared interface, not a module instance, Resource, operation
path or value. Independent compatible providers use the same API declaration.
Human-readable names are documentation; no dot separator or name parsing has
dispatch meaning.

The module exports the key and full declaration. A typed wrapper carries the
same declaration. Binding checks both key and declaration, including layouts,
method families, rights and reference rules. Same layout with another key is a
different semantic interface; same key with a different declaration rejects.
This is exact checking, with no compatibility negotiation or version range.
[ABI](06.1-native-abi.md) owns binary versioning; calls select no schema version.

The API author maintains the key in the API package; no central registration or
new dependency is required. The key is not an authenticity or memory-safety
proof. Loader authorization/code provenance and provider conformance are
separate. Different implementations of Parse can bind; merely naming an export
parse does not make another interface into Parse.

Resource discovery lists offered declarations. The host validates and retains an
offered declaration before executing provider code. Typed binding requests a
specific declaration; dynamic binding selects an offered declaration from that
Resource. Both produce caller-owned access bound to one live implementation.
Bound calls no longer carry an ApiId: the access records the declaration.

Exports/imports supply the full definitions of referenced APIs. Runtime retains
one canonical declaration per live ApiId and rejects conflicting definitions;
it does not trust the first Resource advertising a key as the caller's expected
type. Ref qualification checks that definition, not only its 16-byte key.

## Declaration record

```text
InterfaceDecl
    api: ApiId
    methods: ordered list<MethodDecl>

MethodDecl
    number: u32
    family: Exec | Ioctl
    dispatch: Call | Edit | Factory
    name: ASCII diagnostic/CLI identifier
    subject_access: READ/WRITE/EXEC mask
    activity: allowed READ/WRITE/STATUS event mask
    input: Type
    output: Type
    progress: Type (contains no Ref)
    created: Ref (Factory only)
```

Within one interface, ioctl numbers are unique nonzero integers. Exec has one
entry numbered zero and invokes the executable Resource's selected behavior.
A Resource may offer separate exec/control interfaces; there is no redundant
opcode in an exec call. Dispatch uses the bound interface and ioctl number.
Input, output and progress declarations select codecs; the caller cannot replace
them. UNKNOWN's nonempty evidence uses progress; empty evidence means none,
including Unit when no progress value is needed.
Progress cannot contain Ref, even in an unselected Choice or empty Sequence;
recovery retains ownership in the original request. FAILED has no value or Ref.
activity bounds the provider's known-event report for internal node timestamps;
it is neither a grant nor proof that every successful call changes state.

Call uses ordinary exec/control callbacks. Edit uses the state preparation table
and returns host-owned accepted/observed state metadata plus the declared value.
Factory uses a creation staging table, including creating ioctls such as accept
or watch acquisition; its host result includes created access plus the declared
value. A descriptor cannot route a Buffer edit
through ordinary mutable control state. The corresponding table must exist.

Factory declares exactly one primary created Ref, including its API and initial
held rights. The created Resource must offer that API's canonical declaration;
AnyResource leaves its API unconstrained. Runtime gives the created access exactly
the declared held rights within the caller's creation grant. Factory output
describes its auxiliary value. Call/Edit have no created field. For example,
[Parse](07-domain-apis.md)
declares created: Ref(TreeSnapshot, READ), output: Unit, progress: Unit.

The base type grammar is deliberately independent of domain storage:

```text
Type = Unit | Bool | U8 | U32 | U64 | I64 | F64
     | Bytes(max_bytes) | Utf8(max_bytes)
     | Record(fields: ordered list<(name, Type)>)
     | Sequence(max_items, element: Type)
     | Array(items, element: Type)
     | Choice(variants: ordered list<(tag: u32, name, Type)>)
     | Ref(api: ApiId | AnyResource, access: Access)
     | Identity
```

Identity is a copied Resource identity qualified by its Kernel instance; it
neither grants nor retains access. Ref is an actual access transfer. Input Ref
borrows attenuated access for the admitted request; output Ref returns owned
access. Persistent retention of an input uses the explicit capture operation,
not copying its numeric token. AnyResource leaves the data API unconstrained
but still checks the declared rights and Resource ownership.

Record field order and Choice tags are part of the declaration. Field/variant
names support generic CLI construction, not dispatch. Definitions must be finite
and acyclic; recursive domain trees use bounded node/edge records or Resource
references. Sequence permits runtime-sized data within its declared bound.
Provider declarations specify call/data bounds rather than relying on unbounded
allocations; Runtime limits may be stricter and then reject binding/admission.

The descriptor representation uses fixed tag values assigned by the common ABI,
explicit lengths and this finite grammar. These are protocol primitives, not a
registry of application types. Aliases such as SectionId=u32 live in the API
package; the interface identity and semantic declaration distinguish their use.

## Canonical descriptor bytes

One descriptor starts with ApiId[16], then method_count:u32. Methods are ordered
by (family, number), with no duplicates. Each method is family:u8, number:u32,
dispatch:u8, subject_access:u32, activity:u8, name, input Type, output Type and
progress Type. Factory alone appends created Type, which must be a Ref.
Dispatch 0 is Call, 1 Edit and 2 Factory. Family 1 is Exec and 2 is
Ioctl. All integer fields use little endian; the containing ABI is 0.1.0, so
there is no second descriptor-version negotiation field.

A name is byte_length:u16 followed by that many ASCII bytes, with no trailing
NUL. Empty or duplicate method/field/variant names in the same owner reject.
Type tags and following operands are:

| Tag | Type | Following descriptor operands |
| --- | --- | --- |
| 0..6 | Unit, Bool, U8, U32, U64, I64, F64 | None |
| 7, 8 | Bytes, Utf8 | max_bytes:u32 |
| 9 | Record | count:u16, then each field name and Type |
| 10 | Sequence | max_items:u32, element Type |
| 11 | Choice | count:u16, then each tag:u32, name and Type |
| 12 | Ref | api[16], access:u32 |
| 13 | Identity | None |
| 14 | Array | items:u32, element Type |

Zero api[16] in Ref means AnyResource; a real ApiId is nonzero. Access bits are
READ=1, WRITE=2, EXEC=4; unknown bits reject. Ref with zero rights still retains
owned/scoped access but permits no data or executable operation. This represents
a newly opened handle; Identity instead grants and retains nothing. Exec's
subject mask includes EXEC. Ref rights are independently enforced, never
inferred from the caller's subject mask. Activity bits are READ=1, WRITE=2,
STATUS=4; unknown bits reject. Zero is allowed.

A descriptor is at most 64 KiB and 256 methods; a type has at most 16 nesting
levels, 256 fields/variants and names at most 255 ASCII bytes. These are named
admission bounds, not measured performance guarantees. A caller cannot increase
them in the descriptor. Call byte/reference limits are separate Runtime
configuration and must accommodate the method before it is invoked. Large data
uses Resource access/paged operations instead of giant descriptors.

The host validates the entire descriptor, including trailing bytes and duplicate
Choice tags, before retaining a binding. Type/rights validation does not invoke
module callbacks. Generated wrappers carry the same descriptor bytes; comparison
uses validated content rather than native structure padding or pointer identity.

## Values and encoding

A dynamic caller constructs a Value matching the selected Type. Value contains
only the primitive/record/sequence/choice/reference contents. It contains no
Contract, operation selector, module identity or independently chosen schema.
Typed wrappers use generated encoders/decoders for the same declaration.

Canonical value bytes are: fixed-width little-endian integers; Bool is 0/1;
F64 is IEEE-754 binary64 bits; Unit is empty. Bytes/Utf8 and Sequence begin with
a u32 length. Fixed Array concatenates exactly its declared number of elements
without a length prefix. Records concatenate fields in order. Choice begins
with its declared u32 tag followed by that variant's value. No native padding.
Utf8 rejects malformed encoding; arbitrary source bytes use Bytes instead.
Domain constraints such as finite mesh positions remain provider validation.

Identity encodes a Kernel-instance epoch[16] and object serial:u64. Kernel
instances must use distinct epochs; serials are nonzero and never reused within
one epoch. This public identity is metadata, not authorization or a service
token.

Ref encodes a u32 index into a separate access-reference vector. Runtime checks
the referenced token's owner, rights, API and liveness, then supplies a scoped
provider reference with no greater rights. Every vector entry must be referenced
by the value; undeclared/unused references reject. Repeated use cannot amplify
rights. Identity uses the common instance-qualified identity encoding, not Ref.

Decode rejects unknown tags, malformed lengths, overflow, trailing bytes,
depth/element/byte limits and unavailable reference slots before invocation.
Result decoding uses the bound output Type. A malformed result after a possible
effect is never reported as pre-admission rejection or a reason to repeat it;
the original request retains the provider error and effect-recovery custody.

## Example: a module-owned ELF control API

```text
api: ELF_CONTROL_ID (constant from elf_api)

1 GET_SECTION_COUNT
    family: Ioctl
    dispatch: Call
    subject_access: READ
    activity: READ
    input: Unit
    output: U32
    progress: Unit

2 GET_SECTION
    family: Ioctl
    dispatch: Call
    subject_access: READ
    activity: READ
    input: Record { section: U32 }
    output: Record {
        name: Utf8(256),
        offset: U64,
        size: U64
    }
    progress: Unit
```

This read-only control example can live on a snapshot of parsed ELF state.
The ELF module supplies actual handlers; Kernel knows neither sections nor ELF
layout. An editing control is exposed through the editing preparation boundary,
not by making this callback privately mutate a Buffer.

The optional API package generates Elf::section_count() -> Request<u32> and
Elf::section(SectionId) -> Request<SectionInfo>. Its implementation encodes the
bound ioctl number and declared input. A generic CLI can inspect method 2,
construct record {section: 3}, call it and display the validated result fields
without importing elf_api. It still needs READ on the selected ELF Resource.
