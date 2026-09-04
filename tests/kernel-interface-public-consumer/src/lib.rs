#![no_std]
#![forbid(unsafe_code)]
//! Executed external consumer of Kernel Interface contracts.

extern crate alloc;

mod clock;
mod diagnostic;
mod effect;
mod event;
mod lifecycle;
mod operation;
mod output_health;
mod output_health_state;
mod public_items;

use core::{error::Error, fmt::Debug};

const _: [(); 1] = [(); public_const_cases() as usize];

const fn public_const_cases() -> bool {
    operation::const_cases()
        && lifecycle::const_cases()
        && clock::const_cases()
        && diagnostic::const_cases()
        && effect::const_cases()
        && event::const_cases()
        && output_health::const_cases()
        && output_health_state::const_cases()
}

/// Exercises the admitted Kernel Interface contracts through their public API.
#[must_use]
pub fn exercise_public_api() -> bool {
    public_items::witness()
        && operation::exercise()
        && lifecycle::exercise()
        && clock::exercise()
        && diagnostic::exercise()
        && event::exercise()
        && output_health::exercise()
        && output_health_state::exercise()
}

const fn derive_witness<T>() -> bool
where
    T: ::core::clone::Clone
        + ::core::marker::Copy
        + ::core::fmt::Debug
        + ::core::cmp::PartialEq
        + ::core::cmp::Eq,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

const fn clone_copy_witness<T>() -> bool
where
    T: ::core::clone::Clone + ::core::marker::Copy,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

const fn clone_debug_eq_witness<T>() -> bool
where
    T: ::core::clone::Clone
        + ::core::fmt::Debug
        + ::core::cmp::PartialEq
        + ::core::cmp::Eq,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

const fn send_sync_witness<T>() -> bool
where
    T: ::core::marker::Send + ::core::marker::Sync,
{
    let _layout = (core::mem::size_of::<T>(), core::mem::align_of::<T>());
    true
}

fn traits<T>(value: T) -> bool
where
    T: Clone + Copy + Debug + Eq,
{
    let cloned = clone_value(&value);
    let (left, right) = copy_pair(cloned);
    require_debug(&left);
    left == right
}

fn clone_value<T: Clone>(value: &T) -> T {
    value.clone()
}

const fn copy_pair<T: Copy>(value: T) -> (T, T) {
    (value, value)
}

const fn require_debug<T: Debug>(_value: &T) {}

const fn error_traits<T>(_value: &T) -> bool
where
    T: Error,
{
    true
}
