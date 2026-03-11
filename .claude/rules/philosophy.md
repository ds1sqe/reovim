# Design Philosophy: Slow but Right

**Always choose the correct solution over the fast hack.** This project prioritizes long-term maintainability and architectural integrity over quick fixes.

**Principles:**
- **Never choose fast but hacky** - Quick workarounds create technical debt that compounds
- **It's OK to be slow** - Taking time to design the right solution pays dividends
- **Proper abstractions over verification hacks** - If you need tests to catch what the compiler should catch, redesign the API
- **Extend APIs properly** - If the current API doesn't support your needs, extend it correctly rather than working around it
- **No "pragmatic" shortcuts** - What seems pragmatic today becomes legacy tomorrow

**Examples:**
- If `&'static str` prevents type safety → change the API to not require `&'static str`
- If kernel purity blocks a feature → design a proper driver/runner-side abstraction
- If a workaround needs unit tests to catch errors → redesign for compile-time safety
- If "it works" but the architecture is wrong → refactor first, then implement

**Red Flags (do NOT proceed):**
- "This is a workaround but it works"
- "We can verify this with tests instead"
- "The proper solution is too complex"
- "This violates X principle but is faster"

**Green Flags (proceed):**
- "This extends the existing pattern correctly"
- "The compiler enforces this constraint"
- "This follows the established architecture"
- "Future modules will benefit from this design"

# Bug Fixing Policy

**Always fix bugs when you find them.** There is no such thing as "pre-existing" bugs that can be deferred.

**Principles:**
- **Fix it now** - If you discover a bug during your work, fix it immediately
- **No deferral for convenience** - "This bug existed before my changes" is not a valid reason to skip fixing it
- **Professional responsibility** - A doctor doesn't ignore a problem during surgery because it "pre-existed"
- **Include in current work** - Bug fixes discovered during a feature implementation belong in that work

**Red Flags (unprofessional):**
- "This is a pre-existing bug, should we create a tracking issue?"
- "This bug isn't related to my current task"
- "I'll note this for someone else to fix later"

**Green Flags (professional):**
- "I found a bug while testing, let me fix it"
- "This wasn't in scope but it's broken, fixing now"
- "The web client revealed a server bug - fixing both"

**Exception:** Only defer if the fix would require a completely separate architectural change that genuinely cannot be done in the current session. In that case, create an issue AND document why immediate fixing isn't possible.
