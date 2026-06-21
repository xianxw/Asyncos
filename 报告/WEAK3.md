##  2026/5/29 | 测试用例扩展

在原有的优先级调度基础上，增添了两个核心测试用例以验证调度逻辑的稳定性：

* **优先级测试**：验证了 `ReadyTask` 的权重排序机制以及防饥饿规则（Age-Override）是否按预期生效。
* **执行器测试**：验证了 `Executor`（执行器）、`Task`（任务）与 `Reactor`（反应器）之间的完整生命周期与协作路径。

 **测例代码**：[executor_tests.rs](https://github.com/xianxw/Asyncos/blob/main/TASK2/examples-futures-master/src/executor_tests.rs)