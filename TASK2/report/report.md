# TASK2 协程优先级调度实验报告

## 一、实验目的
验证 `examples-futures-master` 中实现的协程优先级调度机制是否正确，同时检查防饥饿插队逻辑、任务唤醒逻辑和执行器的基本调度流程是否符合预期。

## 二、实验方法
本次实验采用“演示 + 自动化测试”的方式进行验证。

1. 通过 `run_priority_demo()` 进行演示验证，运行命令为 `cargo run`，观察任务在乱序装载后的实际输出顺序。
2. 在 `src/lib.rs` 中通过 `#[cfg(test)] mod executor_tests;` 接入 `src/executor_tests.rs`，并运行 `cargo test` 验证新增的两个单元测试：
	- `test_priority_and_starvation`：构造两个 `ReadyTask`，放入 `BinaryHeap`，检查更老的相邻优先级任务是否会因防饥饿规则优先出队。
	- `test_executor_single_task`：创建 `Reactor` 和 `Executor`，提交一个需要一次定时唤醒的 `Task`，检查任务是否能被正常唤醒、执行并完成。

## 三、实验结果
1. 演示程序输出结果：

	- 任务以乱序方式装载完成。
	- 期望执行顺序为 `Critical(4) -> High(3) -> Normal(2) -> Low(1)`。
	- 实际执行结果依次为：
	  - `Got 4 at time: 3.00.`
	  - `Got 3 at time: 3.01.`
	  - `Got 2 at time: 3.01.`
	  - `Got 1 at time: 3.01.`

2. 自动化测试结果：

	- 在 `TASK2/examples-futures-master` 中执行 `cargo test`，编译完成后输出结果如下：
	  - `src/lib.rs` 运行 2 个单元测试，`test_priority_and_starvation` 与 `test_executor_single_task` 均通过。
	  - `src/main.rs` 运行 0 个测试，结果正常。
	  - `Doc-tests how_futures_are_implemented` 运行 0 个测试，结果正常。
	  - 最终汇总为 `test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`。
	- 优先级测试验证了 `ReadyTask` 的排序与防饥饿规则。
	- 执行器测试验证了 `Executor`、`Task` 与 `Reactor` 的协作路径。

## 四、实验结论
从演示输出和自动化测试结果来看，当前调度器不仅能够按照优先级优先执行高优先级任务，也能够在相邻优先级任务之间通过年龄信息触发防饥饿插队；同时，执行器可以正确处理一次定时唤醒并完成任务收尾。

相比只依赖人工观察输出，本次新增的两个测试把关键行为固定下来，能够更直接地验证 `lib.rs` 中的调度逻辑是否符合设计预期，实验结论也因此更稳定、更可复现。
