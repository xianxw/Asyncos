##  2026/5/23 | 协程扩展：基于优先级的任务调度器实现

通过前一段时间对以下内容的学习：
* **Tokio Future**
* **A stack-less Rust coroutine library under 100 LoC**
* **200行实现协程**（books-futures-explained）

在此基础上，对“200行实现协程”的源代码进行了深度修改，并在其中扩展了**任务优先级支持**。

###  实现功能概述

1. **模块化构建**：将协程核心代码封装并移入 `lib.rs` 文件夹内。
2. **任务优先级与防饥饿机制**：
   * 为任务设置了 **4 个优先级**。
   * 底层的 `reactor` 依然只负责唤醒任务，核心调度交由 `executor` 处理。执行器优先从 `ready_queue` 中弹出优先级较高的任务。
   * **防饥饿处理**：比较任务与任务之间的等待时长，若某个任务等待时间过长且优先级差别不大（最多差一级），则会优先选择等待时长过长的“高龄”任务出列。
3. **功能测试**：编写了基础测试函数 `run_priority_demo`，目前仅用于测试其对普通优先级的支持情况。

###  核心实现逻辑

将源代码中原本的单 Future 驱动器 `block_on`，修改为可维护多个任务的**任务执行器 (Executor)**。并在其中放入就绪任务队列 `ready_queue`（底层基于 `BinaryHeap` 实现）以及用于保存挂起任务状态的全量哈希表 `tasks`。整体架构分为以下四个核心模块：

#### 1. 执行器调度与休眠机制 (Executor & Parker)
* **任务运转**：调度主循环位于 `Executor::run`。执行器不断调用 `pop_ready_task` 尝试从优先队列中取出最高权重的任务，交由 `poll_ready_task` 为其注入上下文（`Context`）并调用 `Future::poll` 推进状态。
* **线程阻塞**：当就绪队列与任务池均为空时，系统退出；若任务池有挂起任务但就绪队列为空，则调用自定义的 `Parker::park`（基于 `Mutex` 和 `Condvar` 实现）让执行器线程陷入深度睡眠，避免 `while` 死循环造成的 CPU 空转。同时利用 `resumable` 标志位解决原生 `thread::park` 易发的虚假唤醒和唤醒信号丢失问题。

#### 2. 动态优先级与防饥饿队列 (ReadyTask & Ord)
* **常规排序**：将任务封装为 `ReadyTask`，并为其实现标准库的 `Ord` trait。在 `ReadyTask::cmp` 函数中，默认先比较 `TaskPriority::score()`，同级情况下通过比较 `enqueued_at` 实现先入先出。
* **老化插队**：为防止低优先级任务饿死，设计了 `age_override_applies` 判定函数。它通过调用 `priority_gap` 和 `age_gap`，判断若两个任务优先级相差不大（≤ 1级）且低优先级任务等待时间超过设定的阈值，则在 `cmp` 比较时强行反转权重，使老任务优先出列。

#### 3. 自定义唤醒器机制 (Waker & VTable)
* **状态打包**：摒弃粗粒度的全局唤醒，通过 `task_waker` 函数将所属的执行器弱引用、`task_id` 和 `priority` 封装进 `TaskWake` 结构体中。
* **虚表构建**：手动实现 `RawWakerVTable` 要求的四个内存与生命周期管理函数（`task_waker_clone`、`task_waker_wake`、`task_waker_wake_by_ref`、`task_waker_drop`），利用 `Arc::into_raw` 和 `mem::forget` 与编译器博弈，最终在 `task_waker_into_waker` 中伪装成标准库可识别的 `Waker` 传呼机。
* **重入队逻辑**：当被底层唤醒时，触发 `TaskWake::wake_task`，其内部调用 `ExecutorInner::enqueue` 将任务重新塞回 `ready_queue`，并立刻调用全局 `Parker::unpark` 惊醒执行器。

#### 4. 后台事件监听与模拟 (Reactor)
* **挂起与注册**：当任务在 `Task::poll` 中发现数据未就绪时，主动调用 `Reactor::register`，将自身的 `Waker` 存入 `Reactor` 的账本中，并向通道发送 `Timeout` 事件。
* **异步唤醒**：`Reactor::new` 初始化的后台主线程监听到事件后，派生模拟 I/O 的小兵线程。耗时操作结束后，小兵线程回调 `Reactor::wake` 方法，通过 `mem::replace` 变更任务状态为 `Ready`，并按下 `waker.wake()` 触发上述的重入队与执行器唤醒链路，完成完整的异步调度闭环。

---

 **完整代码**：[Asyncos/TASK2/examples-futures-master](https://github.com/xianxw/Asyncos/tree/main/TASK2/examples-futures-master)