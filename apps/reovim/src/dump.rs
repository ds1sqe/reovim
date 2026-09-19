//! The core at death: a panic hook that saves everything the Kernel
//! holds where the process stands, as Linux leaves a `core` in the
//! current directory, and then lets the panic go on.

use std::cell::RefCell;
use std::rc::Rc;

use reovim_kernel_runtime::Kernel;

thread_local! {
    /// The Kernel of this thread, for the hook to read. A Kernel is one
    /// thread's, so only a panic on its own thread can take its lock;
    /// a panic on an io thread finds nothing here and writes no core.
    static KERNEL: RefCell<Option<Rc<Kernel>>> = const { RefCell::new(None) };
}

/// `kernel` is the one a panic on this thread saves. The hook wraps
/// whatever hook was installed before, so the panic is still reported
/// and the process still dies.
///
/// The tui installs a hook of its own when it takes the terminal, and
/// that one gives the terminal back before it calls this one, so the
/// path below is read on a terminal that is itself again.
pub fn watch(kernel: &Rc<Kernel>) {
    KERNEL.with(|slot| *slot.borrow_mut() = Some(kernel.clone()));
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        if let Some(path) = saved() {
            eprintln!("reovim: core written to {path}");
        }
        previous(panic);
    }));
}

/// The core of this thread's Kernel, in the file the hook names, or
/// `None` when there is no Kernel here, it could not give a core, or
/// the file could not be written. A hook that failed says nothing and
/// leaves the panic to speak.
fn saved() -> Option<String> {
    let path = format!("reovim-core.{}", std::process::id());
    let kernel =
        KERNEL.try_with(|slot| slot.try_borrow().ok()?.clone()).ok()??;
    written(&kernel, &path).then_some(path)
}

/// Everything `kernel` holds written to `path`; false when the Kernel
/// could not give a core or the file could not be written.
pub fn written(kernel: &Kernel, path: &str) -> bool {
    let Some(core) = kernel.core() else { return false };
    std::fs::write(path, core).is_ok()
}

#[cfg(test)]
mod tests {
    use reovim_client::Rights;

    use super::written;
    use crate::local::Local;
    use crate::server::boot;

    /// The sections a core holds, in the one order it writes them.
    const SECTIONS: [&str; 10] = [
        "limit", "module", "rd", "cell", "decl", "caller", "handle", "event",
        "log", "live=",
    ];

    /// The hook's writer over the server's own Kernel, with a Client on
    /// it so every section has something in it: the file holds the
    /// core, each section once and in order, beginning at the
    /// parameters and ending at the heap.
    #[test]
    fn the_hook_writes_every_section_the_kernel_holds() {
        let dir = std::env::temp_dir()
            .join(format!("reovim-core-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let root = dir.to_str().unwrap();
        let Ok((kernel, _modules)) = boot(root, &[]) else {
            panic!("the server did not boot")
        };
        // A link makes a Session with a capability cell in it, and the
        // handle below is one the core counts.
        let Ok(mut client) = Local::client(&kernel, None) else {
            panic!("no client")
        };
        let log = client.resolve("/sys/log").unwrap();
        let _held = client.open(log, Rights::READ).unwrap();

        let path = dir.join("core");
        let path = path.to_str().unwrap();
        assert!(written(&kernel, path), "the core was not written");
        let text = std::fs::read_to_string(path).unwrap();
        let heads: Vec<&str> =
            text.lines().filter_map(|line| line.split(' ').next()).collect();
        let mut at = 0;
        for word in SECTIONS {
            let rest = &heads[at..];
            let found = rest.iter().position(|head| head.starts_with(word));
            at += found.unwrap_or_else(|| panic!("no {word}: {heads:?}"));
        }
        // `mem` is last, being the one line that moves as a core is
        // made, and it is one line of three numbers.
        let last = text.lines().next_back().unwrap();
        let words: Vec<&str> = last.split(' ').collect();
        assert_eq!(words.len(), 3, "{last}");
        assert!(words[0].starts_with("live="), "{last}");
        assert!(words[2].starts_with("allocs="), "{last}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
