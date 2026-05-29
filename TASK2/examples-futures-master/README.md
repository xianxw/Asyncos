# 《200 行 Rust 讲清 Futures》示例代码

这是配套书籍 [Futures Explained in 200 lines of Rust](https://github.com/cfsamson/books-futures-explained) 的示例仓库。

这个示例展示了一个自包含的 `Future` 实现。除此之外，我们还构建了一个简单的 `Executor` 和 `Reactor`，它们不依赖真实 I/O，因此非常适合动手调试、观察和学习异步运行机制。

所有代码和概念都在配套书中进行了详细说明。

## TASK2：优先级调度扩展

本仓库在原始示例的基础上增加了 TASK2 扩展，用来实现带优先级的任务调度。

- 协程核心代码已经放入 `src/lib.rs`。
- 调度器支持四个优先级：`Low`、`Normal`、`High` 和 `Critical`。
- `Reactor` 仍然只负责唤醒任务，任务选择与调度交给 `Executor` 完成。
- 就绪任务保存在基于 `BinaryHeap` 的 `ready_queue` 中，因此优先级更高的任务会优先弹出。
- 为了降低饥饿问题，如果两个任务的优先级最多相差一级，并且某个任务等待时间过长，则会优先选择等待更久的任务。
- `run_priority_demo()` 是一个非常简单的演示函数，只用于测试普通优先级任务的支持情况。

### 实现概述

- `Executor` 与 `Parker`：执行器主循环持续从队列中取出就绪任务，并为任务注入新的 `Context` 后调用 `Future::poll` 推进状态。当就绪队列和任务池都为空时，执行器退出；当还有挂起任务但就绪队列为空时，使用自定义 `Parker` 进入休眠，避免 `while` 空转造成 CPU 空耗。这里用 `Mutex + Condvar` 和 `resumable` 标志位来处理虚假唤醒和唤醒信号丢失问题。
- `ReadyTask` 与 `Ord`：每个就绪任务都会记录入队时间。比较逻辑会结合任务优先级、同优先级下的 FIFO 顺序，以及基于等待时长的防饥饿插队规则。
- `Waker` 与 `RawWakerVTable`：任务被封装进自定义 `Waker` 中，包含执行器弱引用、`task_id` 和优先级信息。通过手动实现 `RawWakerVTable` 所需的四个函数，并借助 `Arc::into_raw`、`mem::forget` 等方式构造标准库可识别的 `Waker`。
- `Reactor`：反应器负责记录挂起任务、保存其 `Waker`，并通过超时事件模拟异步唤醒。当任务对应的等待结束后，`Reactor` 会把任务状态改为 `Ready`，并调用 `waker.wake()` 触发重新入队和执行器唤醒，形成完整的异步调度闭环。

该扩展已经通过 `src/executor_tests.rs` 中的单元测试进行了覆盖。

**可以探索的分支有：**

- `master`：书中示例代码。`Futures` 可以在 `async_std` 和 `tokio` 上运行。
- `basic_example_commented`：与 `master` 相同，但增加了大量注释，便于理解每一步。
- `bonus_runtimes`：演示使用其他执行器时，示例仍然能够正常运行。
- `vtable`：一个关于胖指针的示例。我们会从原始数据构造 Trait 对象，并实现自己的 vtable。

## 贡献

欢迎提交贡献。小的修正或正确性问题会合并到 `master`，并同步更新到书中；但如果是较大的重写，也需要在书中对应更新。

如果你提交了有趣、不同、值得展示的改进版本，我也会创建新的分支，并在 README 中指向它们。

欢迎在 issue tracker 中提问或讨论。