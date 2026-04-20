#![cfg_attr(coverage_nightly, coverage(off))]
//! Profiler bootstrap.
//!
//! Wires kernel's `Profiler` trait to `tracing` spans so `profile_scope!`
//! calls in server code produce flame-graph data when `REOVIM_PROFILE` is set.
//!
//! Without this, kernel uses `NopProfiler` and `profile_scope!` expansions
//! are zero-overhead no-ops.

use std::cell::RefCell;

use {
    reovim_kernel::api::v1::{Profiler, SetProfilerError, SpanData, SpanId, set_profiler},
    tracing::{Level, Span, span},
};

const PROFILE_ENV_VAR: &str = "REOVIM_PROFILE";

fn profiling_filter() -> Option<String> {
    std::env::var(PROFILE_ENV_VAR).ok()
}

thread_local! {
    static SPAN_STACK: RefCell<Vec<Span>> = const { RefCell::new(Vec::new()) };
}

pub struct TracingProfiler {
    filter: Option<String>,
}

impl TracingProfiler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            filter: profiling_filter(),
        }
    }

    fn matches_filter(&self, target: &str) -> bool {
        match &self.filter {
            None => false,
            Some(f) if f.is_empty() || f == "1" => true,
            Some(f) => target.starts_with(f.as_str()),
        }
    }
}

impl Default for TracingProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Profiler for TracingProfiler {
    fn enabled(&self, target: &str) -> bool {
        self.matches_filter(target)
    }

    fn enter(&self, data: &SpanData) -> SpanId {
        let span = span!(
            target: "reovim::profile",
            Level::TRACE,
            "profile",
            name = data.name,
            target = data.target,
            span_id = data.id.as_u64(),
        );

        SPAN_STACK.with(|stack| {
            let entered = span.entered();
            let span = entered.exit();
            stack.borrow_mut().push(span);
        });

        data.id
    }

    fn exit(&self, _id: SpanId, elapsed_ns: u64) {
        SPAN_STACK.with(|stack| {
            if let Some(span) = stack.borrow_mut().pop() {
                span.record("elapsed_ns", elapsed_ns);
                drop(span);
            }
        });
    }

    fn counter(&self, name: &'static str, value: u64) {
        if self.filter.is_some() {
            tracing::trace!(
                target: "reovim::metrics",
                counter = name,
                value = value,
                "counter"
            );
        }
    }

    fn histogram(&self, name: &'static str, value_us: u64) {
        if self.filter.is_some() {
            tracing::trace!(
                target: "reovim::metrics",
                histogram = name,
                value_us = value_us,
                "histogram"
            );
        }
    }
}

static TRACING_PROFILER: std::sync::OnceLock<TracingProfiler> = std::sync::OnceLock::new();

pub fn init_profiling() -> Result<(), SetProfilerError> {
    let profiler = TRACING_PROFILER.get_or_init(TracingProfiler::new);
    set_profiler(profiler)
}
