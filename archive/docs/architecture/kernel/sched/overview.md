# sched/ - Scheduler

Event loop and async task management.

## Source Location

`lib/kernel/src/sched/`

## Key Types

The scheduler exports the following types:

- `Runtime` — main event loop, owns the executor and priority queue of pending tasks
- `Executor` — drives task execution to completion
- `Task` / `TaskId` — unit of scheduled work and its unique identifier
- `TaskState` — lifecycle state of a task (pending, running, completed, cancelled)
- `Priority` — task priority level controlling dispatch order
- `PriorityQueue` — priority-ordered queue of ready tasks

Note: internal field layouts are not part of the public API and may change.

## Runtime

Main event loop: processes incoming events, drains the priority queue of ready tasks,
and triggers rendering when the display state is dirty.

## PriorityQueue

Tasks are dispatched in `Priority` order — higher-priority tasks run before lower ones.
The `Executor` polls tasks from the queue and drives them to completion.

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Concurrency Reference](../../../contributing/internals/concurrency.md) - Tokio patterns
