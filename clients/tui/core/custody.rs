//! Retained authored input and scalar correspondence, without terminal state.

use crate::input::{Action, Input};
use alloc::collections::VecDeque;
use reovim_kernel_interface::buffer::BufferState;

pub struct RetainedInput {
    pub basis: Option<BufferState>,
    pub cursor: u64,
    pub planned: Option<u64>,
    pub pending: bool,
    pub halted: bool,
    pub queued: VecDeque<Input>,
    pub submitted: Option<Submitted>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Submitted {
    pub basis: BufferState,
    pub action: Action,
}
