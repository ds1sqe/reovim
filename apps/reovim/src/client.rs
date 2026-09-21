//! The Client this binary builds: one Host over a link, with a
//! userland of its own under `/self`.
//!
//! Two of the Modules the server links are loaded here too, from the
//! same generated table, so their names stand under `/self/bin` as
//! well as under `/bin` and a typed line runs the Client's own. Each
//! Module is built once and loaded twice in the bare editor; its
//! `start` takes its home from whichever host gave it one.

use std::rc::Rc;

use reovim_client::{Client, Host, Rvm, bin};
use reovim_host::clock;

use crate::Fail;
use crate::link::MODULES;

/// What the Client loads of its own, by the names `modules` lists:
/// coreutils and sysutils, whose Drivers a typed line runs under
/// `/self/bin`, and search, whose home is `/self/search` on its own.
/// fs and text are the server's, holding what is on its disk.
const MINE: [&str; 3] = ["coreutils", "search", "sysutils"];

/// The Host over `client`: the three Modules loaded under `/self`, the
/// Plugins that serve no region of their own, and `/self/bin`'s `load`
/// and `unload` over the `.rvm` files of this machine.
pub fn over(client: Client) -> Result<Rc<Host>, Fail> {
    let host = Rc::new(Host::over(client, clock::now));
    bin::boot(&host, Rvm::new(Box::new(reovim_kernel_abi::load)))?;
    for (name, boot) in MODULES.iter().filter(|(name, _)| MINE.contains(name)) {
        // The root is the server's disk, which these two never read:
        // what they read they reach through the tree.
        let booted = boot("");
        host.load(name, booted.module, booted.drivers)?;
    }
    // Markdown decorates a `.md` draft wherever one is opened and holds
    // no region, so it is loaded here and not in the table; `unload
    // markdown` takes it away.
    let markdown = reovim_plugin_markdown::boot();
    host.load("markdown", markdown.module, markdown.drivers)?;
    Ok(host)
}
