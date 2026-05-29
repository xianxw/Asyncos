use std::{
    collections::BinaryHeap,
    time::{Duration, Instant},
};

use super::{Executor, ReadyTask, Reactor, Task, TaskPriority};

#[test]
fn test_priority_and_starvation() {
    let mut queue = BinaryHeap::new();
    let now = Instant::now();

    let older_normal_task = ReadyTask {
        task_id: 2,
        priority: TaskPriority::Normal,
        enqueued_at: now - Duration::from_secs(3),
    };
    let newer_high_task = ReadyTask {
        task_id: 1,
        priority: TaskPriority::High,
        enqueued_at: now,
    };

    queue.push(newer_high_task);
    queue.push(older_normal_task);

    let first_out = queue.pop().unwrap();

    assert_eq!(first_out.task_id, 2, "更老的相邻优先级任务应当触发防饥饿插队");
}

#[test]
fn test_executor_single_task() {
    let reactor = Reactor::new();
    let executor = Executor::new();
    let start = Instant::now();

    executor.spawn(Task::new(reactor.clone(), 0, 1, TaskPriority::Normal, 1));
    executor.run(start);

    assert!(executor.tasks.lock().unwrap().is_empty(), "任务应该已经执行完成并从执行器中移除");
    assert!(start.elapsed() >= Duration::from_secs(1), "任务至少应该经历一次定时唤醒");
}