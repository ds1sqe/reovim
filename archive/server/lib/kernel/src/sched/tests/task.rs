use {
    super::super::*,
    std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

// === TaskId tests ===

#[test]
fn test_task_id_new() {
    let id1 = TaskId::new();
    let id2 = TaskId::new();
    assert_ne!(id1, id2);
    assert!(id2.as_u64() > id1.as_u64());
}

#[test]
fn test_task_id_from_raw() {
    let id = TaskId::from_raw(42);
    assert_eq!(id.as_u64(), 42);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_task_id_display() {
    let id = TaskId::from_raw(123);
    assert_eq!(format!("{id}"), "Task(123)");
}

#[test]
fn test_task_id_ordering() {
    let id1 = TaskId::from_raw(1);
    let id2 = TaskId::from_raw(2);
    assert!(id1 < id2);
}

#[test]
fn test_task_id_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(TaskId::from_raw(1));
    set.insert(TaskId::from_raw(2));
    assert!(set.contains(&TaskId::from_raw(1)));
    assert!(!set.contains(&TaskId::from_raw(3)));
}

// === Priority tests ===

#[test]
fn test_priority_constants() {
    assert_eq!(Priority::CRITICAL.as_u32(), 0);
    assert_eq!(Priority::HIGH.as_u32(), 50);
    assert_eq!(Priority::NORMAL.as_u32(), 100);
    assert_eq!(Priority::LOW.as_u32(), 200);
    assert_eq!(Priority::IDLE.as_u32(), 1000);
}

#[test]
fn test_priority_default() {
    assert_eq!(Priority::default(), Priority::NORMAL);
}

#[test]
fn test_priority_ordering() {
    assert!(Priority::CRITICAL < Priority::HIGH);
    assert!(Priority::HIGH < Priority::NORMAL);
    assert!(Priority::NORMAL < Priority::LOW);
    assert!(Priority::LOW < Priority::IDLE);
}

#[test]
fn test_priority_custom() {
    let custom = Priority::new(75);
    assert_eq!(custom.as_u32(), 75);
    assert!(Priority::HIGH < custom);
    assert!(custom < Priority::NORMAL);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_priority_display() {
    assert_eq!(format!("{}", Priority::CRITICAL), "Critical(0)");
    assert_eq!(format!("{}", Priority::HIGH), "High(50)");
    assert_eq!(format!("{}", Priority::NORMAL), "Normal(100)");
    assert_eq!(format!("{}", Priority::LOW), "Low(200)");
    assert_eq!(format!("{}", Priority::IDLE), "Idle(1000)");
    assert_eq!(format!("{}", Priority::new(75)), "Priority(75)");
}

// === TaskState tests ===

#[test]
fn test_task_state_default() {
    assert_eq!(TaskState::default(), TaskState::Pending);
}

#[test]
fn test_task_state_is_pending() {
    assert!(TaskState::Pending.is_pending());
    assert!(!TaskState::Running.is_pending());
    assert!(!TaskState::Completed.is_pending());
    assert!(!TaskState::Failed.is_pending());
}

#[test]
fn test_task_state_is_finished() {
    assert!(!TaskState::Pending.is_finished());
    assert!(!TaskState::Running.is_finished());
    assert!(TaskState::Completed.is_finished());
    assert!(TaskState::Failed.is_finished());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_task_state_display() {
    assert_eq!(format!("{}", TaskState::Pending), "Pending");
    assert_eq!(format!("{}", TaskState::Running), "Running");
    assert_eq!(format!("{}", TaskState::Completed), "Completed");
    assert_eq!(format!("{}", TaskState::Failed), "Failed");
}

// === Task tests ===

#[test]
fn test_task_new() {
    let task = Task::new(|| {});
    assert_eq!(task.state(), TaskState::Pending);
    assert_eq!(task.priority(), Priority::NORMAL);
    assert!(task.name().is_none());
    assert!(task.is_executable());
}

#[test]
fn test_task_with_priority() {
    let task = Task::with_priority(Priority::HIGH, || {});
    assert_eq!(task.priority(), Priority::HIGH);
}

#[test]
fn test_task_with_name() {
    let task = Task::new(|| {}).with_name("test_task");
    assert_eq!(task.name(), Some("test_task"));
}

#[test]
fn test_task_execute() {
    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    let mut task = Task::new(move || {
        executed_clone.store(true, Ordering::SeqCst);
    });

    assert!(task.execute().is_ok());
    assert!(executed.load(Ordering::SeqCst));
    assert_eq!(task.state(), TaskState::Completed);
    assert!(!task.is_executable());
}

#[test]
fn test_task_execute_twice_fails() {
    let mut task = Task::new(|| {});
    assert!(task.execute().is_ok());
    assert!(task.execute().is_err());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_task_cancel() {
    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = Arc::clone(&executed);

    let mut task = Task::new(move || {
        executed_clone.store(true, Ordering::SeqCst);
    });

    task.cancel();
    assert_eq!(task.state(), TaskState::Failed);
    assert!(!task.is_executable());
    assert!(!executed.load(Ordering::SeqCst));
}

#[test]
fn test_task_mark_failed() {
    let mut task = Task::new(|| {});
    task.mark_failed();
    assert_eq!(task.state(), TaskState::Failed);
    assert!(!task.is_executable());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_task_debug() {
    let task = Task::new(|| {}).with_name("debug_test");
    let debug_str = format!("{task:?}");
    assert!(debug_str.contains("Task"));
    assert!(debug_str.contains("debug_test"));
    assert!(debug_str.contains("Pending"));
}

#[test]
fn test_task_unique_ids() {
    let task1 = Task::new(|| {});
    let task2 = Task::new(|| {});
    assert_ne!(task1.id(), task2.id());
}

// === Send + Sync tests ===

#[test]
fn test_task_id_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TaskId>();
}

#[test]
fn test_priority_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Priority>();
}

#[test]
fn test_task_state_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<TaskState>();
}

#[test]
fn test_task_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Task>();
}

// === Coverage: TaskId Default impl ===

#[test]
fn test_task_id_default() {
    let id = TaskId::default();
    assert!(id.as_u64() > 0);
}

// === Coverage: execute on cancelled task (work is None) ===

#[test]
fn test_task_execute_cancelled_returns_err() {
    let mut task = Task::new(|| {});
    task.cancel();
    let result = task.execute();
    assert!(result.is_err());
}

// === Coverage: TaskState Debug/Clone ===

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_task_state_debug_clone() {
    let state = TaskState::Running;
    let cloned = state;
    assert_eq!(cloned, TaskState::Running);
    let debug = format!("{state:?}");
    assert!(debug.contains("Running"));
}

// === Coverage: Priority hash ===

#[test]
fn test_priority_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Priority::HIGH);
    set.insert(Priority::LOW);
    assert!(set.contains(&Priority::HIGH));
    assert!(!set.contains(&Priority::NORMAL));
}

// === Coverage: TaskState Hash ===

#[test]
fn test_task_state_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(TaskState::Pending);
    set.insert(TaskState::Completed);
    assert!(set.contains(&TaskState::Pending));
    assert!(!set.contains(&TaskState::Running));
}

// === MC/DC: execute() on a task that is already in Failed state ===

#[test]
fn test_task_execute_on_failed_task_returns_err() {
    // mark_failed() sets state to Failed (not Pending).
    // Calling execute() on a Failed task exercises the true branch of
    // `if self.state != TaskState::Pending` (line 325) with a distinct
    // non-Pending state (Failed) separate from the Completed case.
    let mut task = Task::new(|| {});
    task.mark_failed();
    assert_eq!(task.state(), TaskState::Failed);

    let result = task.execute();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "task already executed or cancelled");
}
