# Rust 多并发爬虫开发与性能优化日志

---

##  2026/5/18
通过前一段时间的学习，初步完成进程、线程、协程的爬虫程序。

###  核心并发模型实现
* **进程 (Process)**
    * **数据读取**：将所要爬取的网页网址导入 `school.txt` 文件，然后通过 `load_target` 函数将其读取到 `Schools` 数组内。
    * **实现逻辑**：通过 `std::process::Command` 进行进程的创建，通过命令行参数来进行数据的传递。如果检测到参数中包含 `worker` 则为子进程，且 `arg[2]` 与 `arg[3]` 分别是子进程所需的 `name` 与 `url`，将其传入 `fetch_data` 函数则可进行爬取。
* **线程 (Thread)**
    * **数据读取**：将所要爬取的网页网址导入 `school.txt` 文件，然后通过 `load_target` 函数将其读取到 `Schools` 数组内。
    * **实现逻辑**：遍历 `Schools` 数组，利用 `std::thread::spawn` 针对每个学校生成一个新的独立线程。在闭包前使用 `move` 关键字，将 `name`、`url` 和克隆的 `client` 的所有权安全地转移到子线程的上下文中，并调用 `fetch_data` 进行实际的爬取。
* **协程 (Coroutine)**
    * **数据读取**：将所要爬取的网页网址导入 `school.txt` 文件，然后通过 `load_target` 函数将其读取到 `Schools` 数组内。
    * **实现逻辑**：遍历 `Schools` 数组，利用 `tokio::spawn` 针对每个学校生成一个协程任务，通过 `async move` 将所需数据的所有权转移进协程。

---

##  2026/5/19（上午）
###  修改内容
完善修改进程、协程与线程的爬虫程序，将其整合到一个程序内，并进行模块化增强：
* 内部加入 `neicun.rs` 用来检测内存。
* `school.rs` 存放 `load_target` 函数，用以读取学校名称与网址。
* 在 `thread.rs`、`corout.rs` 与 `process.rs` 中加入 `percentile` 函数用以计算 **P50** 与 **P95**。
* 在 `process.rs` 中加入 `run_worker_process` 用以获取子进程输出的内容。
* 加入 `parse_metric` 还原返回单次爬取的记录。

###  源码与运行结果
* **历史版本（未写检测函数的爬虫程序归档至 wrong 文件夹）**：[wrong 源码](https://github.com/xianxw/Asyncos/tree/main/Coroutine/wrong)
* **运行结果**：[result.md](https://github.com/xianxw/Asyncos/blob/main/Coroutine/all/result.md)
* **完整代码**：[all 源码目录](https://github.com/xianxw/Asyncos/tree/main/Coroutine/all)

---

##  2026/5/19（下午）
###  修改内容
基于上午完成的进程、线程和协程爬虫程序，针对**进程和线程总耗时过长、吞吐率过低**以及**进程的内存峰值过高**等问题做出一些性能优化：

1.  **运行时重构**：首先是将 `main` 函数改为无 `async`，通过 `runtime::block_on` 阻塞后再调用协程检测。
2.  **I/O 与业务分离**：修改 `corout.rs`，将其爬取网址内容 `fetch_data` 和写入文件操作 `write_school_data` 进行分离。
3.  **管道异步写入**：通过 `mpsc::channel` 将多个爬取数据传给唯一的写入端，并通过 `oneshot::channel` 返回通知，以此确保依次写入。
4.  *另外还有一些较小改动，不再赘述。*

###  优化后成果
* **运行结果**：[second_result.md](https://github.com/xianxw/Asyncos/blob/main/Coroutine/all/reports/second_result.md)
* **完整代码**：[all 优化版源码目录](https://github.com/xianxw/Asyncos/tree/main/Coroutine/all)