//! Bounded, passive request transitions; payloads stay private.

use reovim_kernel_interface::ErrorCode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Admitted,
    Started,
    Accepted { revision: u64, node: u64 },
    Finished,
    Failed(ErrorCode),
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
