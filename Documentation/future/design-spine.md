# Design Spine — Beyond-0.16

**Status:** Non-normative. The design through-line.

This is the "why" behind the long bet — captured thinking, organized by
theme, not a plan. Where a theme has become a 0.16 goal it has a
normative home in a numbered chapter, noted inline; the rest is
direction past 0.16. The hardware-state-editor and reversibility
material was arrived at twice independently (here and in the device-domain
design now at `04-Domain-Substrate/06-Device-Domains.md`), which is
itself a signal it is right.

---

## I. The architectural through-line

- **Via negativa roadmap.** reovim grows by *removing* assumptions, not
  adding them. 0.15: content is not text (text domain split out). 0.16:
  environment is not a host (freestanding / no_std bare metal). 0.17?:
  execution is not a single machine (multi-server). Each minor version is
  one assumption's funeral. The limit of the sequence is a fully
  self-contained, maintenance-free target ("Mars-safe"). 0.16 being long
  isn't a bug — "host" is the heaviest assumption to bury (SMP, USB,
  framebuffer, device-buffer UX are all the cost of that one assumption).

- **Kernel as mathematical mechanism, not a program.** The kernel is
  closer to a spec / a logging contract than to code — invariant. Buffer =
  algebra of sequence + position; undo tree = persistent data structure;
  EventBus = delivery semantics. If these are *genuinely* mathematical
  objects, verification is done once and stays valid forever (math doesn't
  rot; programs rot because they depend on the world). This is the *only*
  path for "the kernel is infallible" to escape being an unfounded
  assumption. In OS mode the kernel crate must not change a single line —
  platform is physics (absorbed by `arch/`), the kernel stays abstract.
  Terminal to bare metal: the math can't change.

- **The vertical descent.** Most careers go horizontal (more frameworks);
  this one goes vertical: editor → server → kernel → ABI → bare metal →
  SMP → physics. At the bottom of the vertical, no one else's code remains
  — bare metal is the logical limit of single-authorship. `std` is not the
  floor; it's one implementation of contracts (Mutex on futex on kernel
  wait-queue on WFE/SEV on "a core sleeps and wakes" = physics). Going down
  makes the world not more mysterious but more *honest* — magic reduced to
  contracts, one layer at a time.

- **freestanding.** 0.16 = "the freestanding implementation of the editor"
  (C's hosted/freestanding distinction; no_std = freestanding). Already
  boots from an SD-card image in QEMU. Deployment unit = disk image (`dd`,
  not `cargo install`) — Smalltalk-image self-sufficiency, and
  intrinsically robust for a bandwidth-starved target (the distribution
  unit is one image). freestanding nodes can *federate*; dependent nodes
  can only *depend* — a node that can't run alone isn't a node, it's a
  limb. (0.16 OS-mode goal: `01-Architecture/06-OS-Modes.md`.)

## Survival — the corruption model

The harshest constraint the architecture is steered to face. Far-horizon;
it justifies present decisions without being a present goal.

- **The three-layer corruption model.** Math (kernel contract — eternal) /
  Body (memory representation — guarded by CRC + scrubbing) / World (arch +
  drivers — replaceable). Different decay modes, different defenses. The
  math layer's endgame verification is *proof*, not tests (seL4 went
  exactly this way, possible only because the kernel was small enough — the
  zero-external-dep, small kernel keeps that door open; tools: Kani,
  Creusot/Prusti). The body layer: CRC / memory-scrubbing self-heal —
  defends not the math but the math's *flesh* (cosmic-ray bit-flips corrupt
  a logically-perfect kernel: with no protective magnetic field, radiation
  flips RAM bits directly — a single-event upset, SEU — and there is no
  maintainer to fix it remotely). The world layer stays swappable. CRC
  self-heal *narrows the infallibility assumption honestly*: from "kernel
  is infallible" (logic + physics) to "kernel logic is assumed sound; the
  bits holding it are runtime-verified."

- **"Mars-safe" as north star, not destination.** Not the literal goal —
  the *harshest constraint* that pushes design principles to their limit
  (multi-minute latency, bit-flip, no maintenance, single author). An
  editor passing all four is overwhelmingly robust in ordinary conditions
  too. It's the north star for pushing one direction for decades; reach it
  or not, every decision made *facing* it becomes honest. It descends into
  concrete practice: "next test = fuzz the panic-recovery mode tens of
  thousands of times."

## II. Verification & static analysis (horizon)

The current floor (golden set, 100% MC/DC, zero-warning, depgraph hard
rule) is normative in `09-Conformance/` and `10-Development/`. The forward
half:

- **The four-layer verification stack.**
  1. **golden set** (regression) — catches "differs from what I declared
     correct."
  2. **MC/DC 100%** (internal completeness of *written* code) — forces each
     condition's independent effect; catches the condition-combination
     blind spot.
  3. **fuzzing** (inputs the author never wrote) — robustness on undefined
     inputs.
  4. **abstract interpretation** (execution orders / paths never *run*) —
     soundly over-approximates all interleavings; home-turf expertise.
  Shared blind spot of golden-set + MC/DC: both verify "internal
  completeness of what I wrote," neither catches "what I didn't write."
  Golden sets can also *freeze a wrong answer* as correct forever.

- **dmesg-grade observability is the precondition for fuzzing.** Fuzzing
  that finds a crash it can't reconstruct is useless. Observability first,
  random attack second. dmesg models a kernel streaming its internal state
  to a ring buffer — maps onto reovim's kernel/driver/module layering (each
  layer emits structured logs, aggregated in time order). For a
  maintenance-free remote target this is the *only* thread a ground
  engineer can pull. Self-description turns "unimaginable failures" into
  "post-mortem-readable failures": you can't eliminate the blind spot, but
  you can make it *visible when hit*. The system generates the info a user
  would have reported — the single-author substitute for bug reports.

- **"exists is not works."** Undo persist/load existed but was never called
  in production (#761) — infra present but unwired; golden-set and MC/DC
  both missed it because "is this code called?" is a different layer of
  question. Panic-recovery mode risks the *exact* same trap — recovery code
  that only runs at the worst moment, carrying its own bug, corrupting an
  already-broken state, is the most insidious class. Need call-path
  integration asserts / fuzzing to verify exists-vs-works. fuzzing + the
  existing panic-recovery + dmesg aggregation mutually validate: fuzzing's
  induced panic is the recovery mode's first real test; the recovery mode
  is fuzzing's first real consumer.

- **A narrow constitution checker, not a general analyzer.** With
  abstract-interpretation expertise in hand, the usual cost calculus
  ("a general analyzer is a second 30-year project, don't") flips. Build a
  narrow, fast checker for reovim's *constitution* — the rules no
  off-the-shelf tool knows ("modules use only `reovim_editor_core::api::*`",
  "kernel has zero external deps", "platforms don't depend on drivers", and
  above all add-only/change-safe). Linus built `sparse` for exactly this.
  The depgraph probe is already this — a hard rule (blocks merge),
  constitutional law, with the political "rule violation halts progress"
  consensus already free because it's single-author. Ladder:
  - **1. depgraph hard rule** — done; the real acceptance criterion is
    *transitional whitelist length 0* (a hard rule with an exception list
    is "law with a grace period").
  - **2. API surface diff** — add-only enforced by rustdoc-JSON diff (build
    on cargo-semver-checks). Rule-based, no abstract interpretation,
    cheapest, max return. This is "don't break userspace" moved from human
    attention to machine law — the single-author equivalent of Linux's
    crowd.
  - **3. MIR abstract-interpretation domains** — lock discipline, position
    algebra (interval domain → off-by-one class statically eliminated),
    panic-freedom (unwind reachability), no-alloc / `#[bounded]`. The
    home-turf weapon; align with the SMP landing (concurrency is exactly
    where golden-set / MC/DC / fuzzing all leak due to nondeterminism —
    abstract interpretation covers all interleavings at once).

- **no_std MIR analysis via contract, not discovery.** MIR alone can't see
  custom spinlocks, `asm!` blocks, or interrupts (implicit control flow
  absent from the CFG). Don't discover semantics — *check declared
  contracts*, the sparse model: annotate the API
  (`#[acquires]`/`#[releases]`/`#[irq_off]`), treat `asm!`-terminal
  functions as **axioms** (small, hand-verified — TCB philosophy). Two
  layers: axiom layer (hardware-touching, hand-checked) + composition layer
  (pure Rust, machine-verified). Forbid raw atomics / `asm!` outside the
  sync crate via a depgraph hard rule → narrowing the language surface makes
  the analysis *sound*. "Forbid arbitrary code in order to make it
  analyzable." no_std is an *advantage*: closed world, all reachable code is
  the author's. (depgraph = layer 1 becomes the soundness precondition for
  layer 3.)

- **Kernel integration testing across substrates.** Split logic from
  substrate (std's own Mutex/Condvar already does this: platform-agnostic
  logic over a futex-shaped parking contract). Verify the pure
  parking-contract logic on the host with **loom** (model-checks all
  interleavings under C11 — fills the concurrency gap execution can't
  reach). Verify the real system level (interrupts, context switch,
  WFE/SEV) in **QEMU** (emulated GIC delivers real interrupts; semihosting
  / magic-MMIO for pass/fail; **SGI** for deterministic interrupt injection
  at an exact instruction; register-pattern preservation for context-save
  integrity). Then **substrate becomes a test-matrix axis**: one contract
  suite, run 3× (mock/loom, futex/Linux, WFE+SEV/QEMU→Pi). "The environment
  differs" stops being an obstacle and becomes the axis — golden-set
  philosophy rotated onto the substrate axis, as add-only was rotated onto
  the version axis.

## III. Real-time & controlling physics

The RTOS-itself mode is a 0.16 OS-mode goal
(`01-Architecture/06-OS-Modes.md`); the *promise-type vocabulary* and the
WCET-domain weapon below are forward design.

- **RTOS vs OS = type of promise, not a feature.** GPOS: statistical ("fast
  on average, fair"). RTOS: worst-case bound ("within this time even
  worst-case"). Express it in the upper layer as a *promise type*, not a
  flag. Each kernel mechanism declares its time contract (O(1)+WCET bound vs
  amortized/no-bound). Admission model: policy declares a deadline class,
  kernel accepts/rejects. Keep the surface *substrate-parametric*: same API
  is a hint (soft) on Linux, a guarantee (hard) on bare metal. The editor
  is a soft-RT system — vocabulary = deadline classes: input ≈ hard (the
  "fastest-reaction" identity), frame render = firm (miss → drop that
  frame), syntax = soft, indexing = background. mechanism/policy rotated
  onto the time axis.

- **Controlling physics = converting unbounded → bounded at each
  boundary.** A transducer at every entry point physics uses: interrupts →
  top-half (minimal deterministic: flag + enqueue) + bottom-half under
  budget + NAPI-style demotion to polling under flood; allocation →
  forbidden in bounded paths + preallocated pools (statically enforced by a
  `#[bounded]` analyzer domain); time → monotonic clock = mechanism,
  deadline = first-class API. **Priority inheritance/ceiling in the
  spinlock is not decoration** — the 1997 Mars Pathfinder reset *was*
  priority inversion (low task holding a mutex preempted by mid, starving
  high until watchdog; fixed by enabling priority inheritance remotely). A
  real flight bug is literally on the long-term TODO list. Industry tool for
  bounding the time physics: **abstract interpretation** (AbsInt aiT,
  DO-178 WCET on a cache/pipeline model) — again converges on home turf.
  Short term: no-alloc / bounded-loop domain (easy). Long term: WCET domain
  over a Cortex-A72 model (hard, but the home-turf weapon itself).

## IV. Hardware-level state editor & reversibility

**Graduated:** the concrete contract is normative at
`04-Domain-Substrate/06-Device-Domains.md` (effect typing, acquire/render
split, commit barrier, capability traits). Retained here as the vision the
contract serves.

- **The logical terminus of the descent.** When the editor becomes the OS,
  "edit a file" and "edit system state" merge. Buffer = abstraction of
  *addressable mutable state* → memory, registers, MMIO, GPIO pins are all
  buffer instances. Push the substrate-independence of position-algebra +
  undo-tree to the limit: `hjkl` to navigate physical memory, `x` to kill a
  bit, `u` to rewind hardware — what a debugger does crudely, done in
  modal-editing grammar. Precedents: Plan 9 ("everything is a file"),
  Smalltalk image (system edits itself live), JTAG/OpenOCD (surgical
  CPU-state editor) — but *none* put an undo-tree over hardware.
  Transactional hardware editing is the empty seat; only a
  persistent-data-structure undo kernel can take it.

- **Physics's counterattack — the reversibility type.** Not all state
  rewinds. RAM is reversible. MMIO *read* can have a side effect (reading
  pops a FIFO). *Write* can launch a DMA — un-launchable. So the buffer
  contract needs a reversibility-type field: `reversible` (RAM, file) /
  `volatile` (read has a side effect, no snapshot) / `irreversible` (write
  changes the world; the undo tree plants a "no return" marker here). The
  undo tree generalizes to a **history tree**: an undo node is `(delta,
  inverse-delta)`; an irreversible change is `(delta, ⊥)`; the history DAG
  stays uniform, only *backward traversal* is gated per edge. You can still
  navigate history on irreversible buffers (like `git log` on a repo you
  can't `reset`) — you just can't checkout past states.

- **Semantics belong in the buffer trait, not the renderer.** Read-purity
  (pure/effectful), write-semantics (W1C, write-only), reversibility,
  access-width — all *declared contract fields*. The renderer stays generic
  and never knows hardware exists: "this buffer declares its read is
  effectful → don't auto-read." Bonuses: (a) the text buffer becomes the
  trivial instance (pure read, reversible write, no constraints) = the
  zero-point of the new axis → add-only preserved → a signal the
  abstraction is right; (b) it feeds the analyzer — "no `read()` on an
  effectful-read buffer in an auto-refresh path" becomes statically
  checkable (sparse pattern: machine checks the declared contract; hide the
  semantics and there's nothing to check). Precedent: `svd2rust` bakes SVD
  access semantics into types (an RO register has no `write` method); the
  reovim version lifts it one step further to *runtime-queryable trait
  properties* used by the generic machinery (renderer/undo/motion) **and**
  the compile-time analyzer — same declaration used twice. CMSIS-SVD is the
  industry serialization of reversibility types (read-only / write-only /
  clear-on-read / write-1-to-clear).

- **"observing is touching."** In hardware buffers the text assumption
  "reading is harmless" breaks — a naive renderer polling a clear-on-read
  FIFO *destroys data by displaying it*. Almost quantum-mechanical UX. The
  render contract must reference access type; effectful registers get a
  placeholder ("read has side effect — `gr` to read explicitly"); reading
  becomes a deliberate act.

- **Write-delta journal = graceful degradation of undo.** Where rewind is
  impossible, demote to recording (aircraft black box). A write journal
  needs no reads (you know what you wrote) → safe even for effectful-read
  regions, whereas a full dump is dangerous (empties the FIFO). And
  **replay (not rewind) works**: reset device + replay journal → reproduces
  the state sequence → a debug session converts *itself* into a QEMU
  golden-set regression. This closes the "quality without bug reports"
  single-author loop. Gate behind debug mode (the journal costs hot-path
  latency; out of `#[bounded]` paths or within budget).

- **vi grammar maps cleanly onto hardware state.** visual-block = bitfield
  ops (select a range, `r1`/`r0`); macros (`qq…q`) = hardware-sequence
  scripting (record an init sequence); dot-repeat = repeated poke; text
  objects (`if` = "inner field") generalize to register fields; yank a
  hardware register value into a vi register. A 40-year editing grammar
  reused as a bit-manipulation language — no new UI language to invent.

## V. Distributed model = git (0.17+)

The over-abstraction line is HERE: multi-server has 0 experience.
Direction recorded; the contract is built only after it hits reality.

- **content-addressing solves distributed identity.** ID = hash of content
  + history → no coordination, structurally collision-free; identity moves
  from the author's *act* to the content's *property*. The undo tree is
  *isomorphic* to a commit DAG → add Merkle-ization → distributable.
  Session = repository; one device = working copy, the big server = origin
  *by convention, not privilege*; every node a complete replica. A long
  disconnection = just a long-lived branch; reconnection = merge. CAP: an
  editor must pick AP (local-first — typing can't stop because the network
  dropped).

- **Merge semantics: git over CRDT.** git surfaces conflicts honestly; CRDT
  guarantees convergence but not intent-preservation. The use model is "one
  author, many devices" → concurrent-edit conflicts are structurally rare,
  and explicit resolution fits "explicit over implicit." CRDT can be
  layered later as a per-buffer-type *policy* when real multi-user
  real-time is needed (merge strategy = policy, DAG = mechanism).

- **DTN Bundle Protocol (RFC 9171)** is the real delay-tolerant transport;
  "transport is policy" means the kernel stays untouched and it can be
  plugged in later. Linus built git for disconnected kernel development — so
  kernel architecture, sparse, *and* git are all artifacts being
  re-walked, because solving the same problem (single author, longevity,
  distribution) yields the same answers.

- **The over-abstraction line is HERE.** multi-server has 0 experience →
  stop. Designing merge-units now = designing the unlived. Record direction
  (`git-like`), build the contract only after it hits reality. Build
  freestanding first.

## VI. Governance ported from Linux

The current ABI doctrine (add-only, `Do not break userspace`) is normative
in `06-ABI/` and the README posture. The governance *system* below is
forward direction.

- **Linux's real success is UX — for the right user.** "don't break
  userspace" is the ultimate UX guarantee; the sacred "user" is the
  software *ecosystem*, not humans. Won where the direct user is an engineer
  (server/embedded) or where someone layered consumer policy on top (Android
  = mechanism/policy split at planetary scale: Linux = unbreakable
  mechanism, Google = human-facing policy). Lost only the "outsource your
  policy" market. **reovim's target = people who want to own their tool's
  policy** (the Hyprland/ricing population) — so beating VSCode's mass
  market is unnecessary and irrelevant. And reovim is *vertically
  integrated*: it owns **both** layers (ABI stability = developer UX; vi
  grammar + latency promise = human UX) — Linus *and* the Android layer at
  once. Single-authorship makes that ambition possible.

- **"pull and it just works" — the ultimate editor.** Every mechanism
  (golden set, MC/DC, zero-warning, depgraph hard rule, ABI add-only)
  exists to manufacture one sentence: *pulling won't betray you*. The
  chronic modern UX failure is *update fear* (pin versions, read changelogs
  defensively, wait for `.1`). The ultimate UX is the *absence of fear*, not
  a feature. Trust can't be declared, only accrued — one unbroken release =
  one coin; one broken release burns years. Linus's authority and TeX's
  40-year binaries are the same balance. The slogan scales: freestanding →
  "insert the SD card and it boots"; remote target → "uplink and it
  survives."

- **Empirical vs formal enforcement.** Linux UAPI is add-only, enforced
  *empirically*: "if someone screams, it's ABI" (Linus is a court, not a
  code; `stable_api_nonsense.txt` explicitly refuses *internal* API
  stability). Empirical enforcement presupposes a screaming crowd. reovim
  has no crowd (no bug reports). So replace the crowd's screams with a
  static checker — Linux: millions of post-hoc reports; reovim: one
  analyzer's pre-emptive block. Same constitution, opposite enforcement.

- **The stable-ABI bet is the opposite of Linus's — and correct at this
  scale.** Linus: "no stable module ABI, come in-tree." The reovim bet
  (#769): versioned ABI promise to drivers + loader verifies pre-exec + load
  failure doesn't kill the process + signed-native / WASM-sandbox two-tier.
  The recurring NVIDIA-update fear is its *negative-space spec*. At Linux
  scale Linus is right (tens of thousands of contributors must evolve the
  API); at single-author + external-extension scale, the stable-ABI promise
  is right. Same problem, different scale, opposite answer. The bet's own
  value is its self-diagnostic honesty: it names the current "plugin" story
  as fiction — adding a module today still means editing the launcher's
  Cargo.toml and recompiling, the launcher is a god-bin, and "independent
  per-component execution" is structurally false. The versioned-ABI +
  runtime-load work is the deliberate, "Slow but Right" retirement of that
  whole class, not a patch on a symptom.

- **The taint model — approve rules, not instances.** Approval doesn't
  scale (the limit is *responsibility* anxiety: "if an unseen type
  misbehaves in my system, is it my fault?"). Linux's answer: the **taint
  flag** — out-of-tree modules need no approval; the kernel marks *itself*
  tainted and tainted-kernel bug reports get "reproduce without the blob." A
  mark, not a gate — it draws the responsibility line automatically.
  Distribute the single author's overloaded approval across **four
  mechanisms**: identity = **namespace** (`vendor.module.Type` /
  content-addressing — structurally collision-free, no approval needed);
  safety = **sandbox** (WASM); responsibility = **taint**; trust =
  **signature** (scarce, deliberate → becomes a *quality signal*: "author
  signature = reviewed atom-by-atom"). Tiers: T0 core / T1 signed / T2
  unsigned (no approval; sandbox + taint + namespace). Signature becomes
  scarce → valuable; the single author goes from *bottleneck* to *issuing
  authority*. Essential for a 30-year project — the author's attention is a
  finite resource.

- **Taint contaminates artifacts.** A debug delta-journal recorded under
  taint must NOT be promotable to a golden set (no evidentiary capacity).
  The journal header carries a taint bit; the test-asset conversion path
  mechanically rejects it — the journal version of Linux refusing
  tainted-kernel bug reports. Protects golden-set purity in a crowdless
  ecosystem. (The dev box already shows `Tainted: P` — proprietary NVIDIA.
  Having lived for years as the *governed* under taint, moving to the
  *issuer* end is just relocating the same contract — the meta-pattern of
  the whole design: porting governance tech experienced as Linux's subject
  into the new kingdom — kernel architecture, sparse, git, taint.)

- **NVIDIA as anti-reference architecture.** "A platform owner's character
  shows in how it treats leverage-less users." NVIDIA abandons non-revenue
  users (upstream desktop) and listens only to screams with a purchase order
  attached (its 2022 open-sourcing was driven by hyperscalers, not desktop
  screams — proving the thesis; AMD's in-tree `amdgpu`/Mesa is the
  contrast). reovim's users — the ones who file no bug reports, the future
  module authors — are *all* leverage-less. Building golden sets and ABI
  promises for people who can't scream is the anti-NVIDIA line, and the only
  place the "pull and it just works" trust balance accrues. Hatred as
  reference architecture: not what to copy, but what to *never* do.

- **Stability boundary, settled.** User-facing UX + user-mode ABI = sacred;
  in-tree internals = free; third-party = taint. UX stability needs
  *mechanization* in two axes: framebuffer golden set (behavior — already
  have it) + perf baseline (time — promote `perf/` reports from report to
  *gate*: kernel-op latency regression vs baseline = UX break = build block;
  "fastest-reaction" demands the time axis too). The ABI version is the
  *airlock* between a free interior and a frozen exterior: outside
  (published version) is add-only, inside traits churn freely.

## VII. Project-level discipline

- **The author-as-only-user blind spot.** No bug reports = either nobody
  uses it or it doesn't break for users; reovim is the latter (the author
  broke everything before users could). The trap: the author's stress test
  = "harshness I imagined," but real users break it via "no one would use it
  that way." A maintenance-free remote target is dangerous precisely because
  it breaks in unimagined ways with no one to fix it. The honest response:
  not eliminate the blind spot but make it *visible when hit* (dmesg) and
  let machines hit beyond imagination (fuzzing, property tests, abstract
  interpretation).

- **Over-abstraction discipline (rule of three, defensive timing).** "This
  feels like doing at 0.16 what I already did once at 0.11" is the
  *strongest* proof it's NOT over-abstraction — abstraction earned by
  experience that *precedes* it (panic recovery justified because done once
  before), vs over-abstraction = generalizing for the not-yet-happened. The
  line is drawn explicitly: device-buffer UX is in-scope (existing contract
  + existing target + needed-anyway); multi-server merge-units are out (0
  experience). Record direction; build the contract only after it hits
  reality — that's the order that survives 30 years.

- **One enormous project for decades, not many trivial ones.** One point bet
  all-in compounds with time into an ecosystem/standard/promise — which is
  why ABI discipline matters from day one (small projects don't need it).
  The real project with full authorship (reovim) lives outside any single
  employer; that independence is itself what makes the long bet possible.

---

## Cross-cutting pattern

Almost every technical decision resolves, when chased to the end, into
**UX**: "fastest-reaction," zero-flicker, "SD card boots it," vi-keymap
device editing, RT promise = "finger-to-photon latency, quantified." The
stack descent doesn't move *away* from the user — it's *forced by* refusing
to compromise UX (you can't promise a latency bound on someone else's host,
so you go to bare metal). Most products do UX at the skin; this does UX
from the physics up — the terminus of the descent is not hardware but the
user's fingertips. mechanism/policy split is a UX doctrine in disguise:
mechanism = UX reliability, policy = UX freedom.
