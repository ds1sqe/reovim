//! Bounded, passive request transitions; payloads stay private.

use core::fmt::Write as _;
use reovim_kernel_interface::ErrorCode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Admitted,
    Started,
    Accepted { revision: u64, node: u64 },
    Finished,
    Failed(ErrorCode),
    Unknown(ErrorCode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    pub sequence: u64,
    pub request: u64,
    pub scope: u64,
    pub subject: u64,
    pub stage: Stage,
    pub parent: Option<u64>,
}

use alloc::{boxed::Box, format, rc::Rc, string::String};
use reovim_kernel_interface::{
    Contract, Error, Message,
    buffer::integer,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};

impl crate::Runtime {
    /// Exposes the bounded request log as an explicitly granted Resource.
    /// ```
    /// use reovim_kernel_runtime::{Limits, Runtime};
    /// let mut runtime = Runtime::new(1, Limits::default())?;
    /// let provider = runtime.diagnostic_resource();
    /// runtime.register("kernel/log", provider).map_err(|(error, _)| error)?;
    /// # Ok::<(), reovim_kernel_interface::Error>(())
    /// ```
    /// Read `kernel.diagnostics` v1 with an optional u64 sequence cursor.
    /// Replies contain text, dropped-event count and the next cursor.
    #[must_use]
    pub fn diagnostic_resource(&self) -> ResourceProvider {
        ResourceProvider::new(
            Rc::downgrade(&self.shared),
            ResourceOps {
                unload: None,
                read: |shared, _, input| {
                    let result = shared
                        .upgrade()
                        .ok_or(Error::CLOSED)
                        .and_then(|shared| describe(&shared.borrow(), input));
                    Box::pin(core::future::ready(result))
                },
                write: |_, _, _| {
                    Box::pin(core::future::ready(Err(Error::DENIED)))
                },
            },
            Box::new([Surface {
                contract: Contract::new("kernel.diagnostics", 1),
                methods: Permissions::READ,
                input: Box::new([]),
                output: Box::new([]),
            }]),
            Rc::new(()),
        )
    }
}

fn describe(shared: &crate::Shared, input: Message) -> Result<Message, Error> {
    let after = if input.data.is_empty() { 0 } else { integer(&input.data)? };
    let mut next = after;
    let mut text = format!("lost={}\n", shared.lost_events);
    for event in shared.events.iter().filter(|event| event.sequence > after) {
        let line = line(event);
        // Leave space for the cursor and hex framing of this bounded page.
        if text.len() + line.len() > 16_000 {
            break;
        }
        text.push_str(&line);
        next = event.sequence;
    }
    writeln!(text, "next={next}").map_err(|_| Error::PRESSURE)?;
    Ok(Message::new(input.contract, text.as_bytes()))
}

fn line(event: &Event) -> String {
    // Private Resource IDs and caller namespaces are not text-encoded handles.
    format!(
        "{} request={} parent={:?} {:?}\n",
        event.sequence, event.request, event.parent, event.stage
    )
}
