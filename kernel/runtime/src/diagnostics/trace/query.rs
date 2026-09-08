use alloc::vec::Vec;
use core::mem::size_of;

use reovim_kernel_interface::cancellation::DeadlineState;
use reovim_kernel_interface::event::editing::EditingStage;
use reovim_kernel_interface::event::trace::{
    TraceCut, TraceError, TraceGap, TraceLocation, TraceNode, TraceQueryBudget,
    TraceReport, TraceStart,
};

use super::{State, capacity};

struct Walk {
    report: TraceReport,
    frontier: Vec<(usize, u64)>,
    seen: Vec<bool>,
    node_limit: usize,
    gap_limit: usize,
    work: u64,
    edges: u64,
}

pub(super) fn walk(
    state: &State,
    start: &TraceStart,
    budget: &TraceQueryBudget,
) -> Result<TraceReport, TraceError> {
    if budget.deadline == DeadlineState::Reached {
        return Err(TraceError::Deadline);
    }
    let mut walk = setup(state, budget)?;
    for (index, node) in state.nodes.iter().enumerate() {
        if !spend(&mut walk) {
            break;
        }
        if matches(node, start) {
            enqueue(&mut walk, index, 0);
        }
    }
    let mut head = 0;
    while let Some((index, depth)) = walk.frontier.get(head).copied() {
        visit(&mut walk, state, index, depth, budget);
        head += 1;
    }
    if walk.report.nodes.is_empty() && walk.report.cut == TraceCut::Complete {
        walk.report.cut = absent(state, start);
    }
    Ok(walk.report)
}

fn setup(state: &State, budget: &TraceQueryBudget) -> Result<Walk, TraceError> {
    let nodes = capacity(budget.nodes.value())?.min(state.nodes.len());
    let gaps = capacity(budget.edges.value())?
        .min(state.nodes.len().saturating_mul(2));
    let predicted = charge(nodes, gaps, nodes, state.nodes.len())?;
    if predicted > capacity(budget.bytes.value())? {
        return Err(TraceError::Pressure);
    }
    let mut report = TraceReport {
        nodes: reserve(nodes)?,
        gaps: reserve(gaps)?,
        cut: TraceCut::Complete,
        oldest: state.nodes.front().map(|node| node.location),
    };
    let frontier = reserve(nodes)?;
    let mut seen = reserve(state.nodes.len())?;
    let actual = charge(
        report.nodes.capacity(),
        report.gaps.capacity(),
        frontier.capacity(),
        seen.capacity(),
    )?;
    if actual > capacity(budget.bytes.value())? {
        return Err(TraceError::Pressure);
    }
    seen.resize(state.nodes.len(), false);
    if budget.nodes.value() == 0 {
        report.cut = TraceCut::Budget;
    }
    Ok(Walk {
        report,
        frontier,
        seen,
        node_limit: nodes,
        gap_limit: gaps,
        work: budget.work.value(),
        edges: budget.edges.value(),
    })
}

fn visit(
    walk: &mut Walk,
    state: &State,
    index: usize,
    depth: u64,
    budget: &TraceQueryBudget,
) {
    let node = state.nodes[index];
    walk.report.nodes.push(node);
    if missing_primary(&node) && budget.primary {
        cut(walk, TraceCut::NotCaptured);
    }
    for parent in parents(&node, budget).into_iter().flatten() {
        follow(walk, state, parent, depth, budget.depth.value());
    }
}

fn follow(
    walk: &mut Walk,
    state: &State,
    parent: TraceLocation,
    depth: u64,
    limit: u64,
) {
    if depth >= limit || walk.edges == 0 {
        cut(walk, TraceCut::Budget);
        return;
    }
    walk.edges -= 1;
    let index = find(walk, state, parent);
    if let Some(index) = index {
        enqueue(walk, index, depth + 1);
        return;
    }
    if walk.report.cut == TraceCut::Budget {
        return;
    }
    let cause = absent(state, &TraceStart::Event(parent));
    cut(walk, cause);
    if walk.report.gaps.len() < walk.gap_limit {
        walk.report.gaps.push(TraceGap { location: parent, cause });
    } else {
        cut(walk, TraceCut::Budget);
    }
}

fn find(
    walk: &mut Walk,
    state: &State,
    location: TraceLocation,
) -> Option<usize> {
    for (index, node) in state.nodes.iter().enumerate() {
        if !spend(walk) {
            return None;
        }
        if node.location == location {
            return Some(index);
        }
    }
    None
}

fn enqueue(walk: &mut Walk, index: usize, depth: u64) {
    if walk.seen[index] {
        return;
    }
    if walk.frontier.len() == walk.node_limit {
        cut(walk, TraceCut::Budget);
        return;
    }
    walk.seen[index] = true;
    walk.frontier.push((index, depth));
}

const fn spend(walk: &mut Walk) -> bool {
    if walk.work == 0 {
        cut(walk, TraceCut::Budget);
        return false;
    }
    walk.work -= 1;
    true
}

const fn cut(walk: &mut Walk, cause: TraceCut) {
    if matches!(walk.report.cut, TraceCut::Complete)
        || matches!(cause, TraceCut::Budget)
    {
        walk.report.cut = cause;
    }
}

fn matches(node: &TraceNode, start: &TraceStart) -> bool {
    match start {
        TraceStart::Event(location) => node.location == *location,
        TraceStart::Request { session, request } => {
            node.context.session == Some(*session)
                && node.context.request == Some(*request)
        }
        TraceStart::Update(identity) => node.context.update == Some(*identity),
    }
}

fn parents(
    node: &TraceNode,
    budget: &TraceQueryBudget,
) -> [Option<TraceLocation>; 2] {
    [
        node.context.parent.filter(|_| budget.primary),
        node.context.additional_parent.filter(|_| budget.joins),
    ]
    .map(|sequence| {
        sequence.map(|sequence| TraceLocation {
            stream: node.location.stream,
            sequence,
        })
    })
}

const fn missing_primary(node: &TraceNode) -> bool {
    use EditingStage::{Committed, Installed, Prepared, Received, UpdateReady};
    node.context.parent.is_none()
        && matches!(
            node.stage,
            Prepared | Committed | UpdateReady | Received | Installed
        )
}

fn absent(state: &State, start: &TraceStart) -> TraceCut {
    if let TraceStart::Event(location) = start
        && state
            .evicted_through
            .is_some_and(|last| location.sequence.value() <= last.value())
    {
        return TraceCut::Evicted;
    }
    TraceCut::NotCaptured
}

fn reserve<T>(count: usize) -> Result<Vec<T>, TraceError> {
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| TraceError::Pressure)?;
    Ok(values)
}

fn charge(
    nodes: usize,
    gaps: usize,
    frontier: usize,
    seen: usize,
) -> Result<usize, TraceError> {
    nodes
        .checked_mul(size_of::<TraceNode>())
        .and_then(|total| {
            gaps.checked_mul(size_of::<TraceGap>())
                .and_then(|gaps| total.checked_add(gaps))
        })
        .and_then(|total| {
            frontier
                .checked_mul(size_of::<(usize, u64)>())
                .and_then(|frontier| total.checked_add(frontier))
        })
        .and_then(|total| total.checked_add(seen))
        .and_then(|total| total.checked_add(size_of::<Walk>()))
        .ok_or(TraceError::Pressure)
}
