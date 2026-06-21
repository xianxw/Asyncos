2026/5/18
通过前一段时间的学习,初步完成进程、线程、协程的爬虫程序.
进程:
数据读取:将所要爬取的网页网址导入school.txt文件,然后通过load_target函数将其读取到Schools数组内
实现逻辑:通过std::process::Command进行进程的创建,通过命令行参数来进行数据的传递,如果检测到参数中包含worker则为子进程,且arg[2]与arg[3]分别是子进程所需的name与url,将其传入fetch_data函数则可进行爬取.
线程:
数据读取:将所要爬取的网页网址导入school.txt文件,然后通过load_target函数将其读取到Schools数组内
实现逻辑:遍历 Schools 数组,利用 std::thread::spawn针对每个学校生成一个新的独立线程.在闭包前使用move关键字，将name、url 和克隆的 client 的所有权安全地转移到子线程的上下文中，并调用 fetch_data 进行实际的爬取.
协程:
数据读取:将所要爬取的网页网址导入school.txt文件,然后通过load_target函数将其读取到Schools数组内
实现逻辑:遍历 Schools数组，利用 tokio::spawn针对每个学校生成一个协程任务,通过 async move将所需数据的所有权转移进协程.
2026/5/19
修改内容: 
完善修改进程、协程与线程的爬虫程序,将其整合到一个程序内,并在内部加入neicun.rs用来检测内存,school.rs存放load_target函数用以读取学校名称与网址,在thread.rs、corout.rs与process.rs中加入percentile函数用以计算p50与p95,在process中加入run_worker_process用以获取子进程输出的内容,加入parse_metric还原返回单次爬取的记录
将首次的三个未写检测函数的爬虫程序放入wrong文件夹:
https://github.com/xianxw/Asyncos/tree/main/Coroutine/wrong 
运行结果:
https://github.com/xianxw/Asyncos/blob/main/Coroutine/all/result.md 
完整代码:
https://github.com/xianxw/Asyncos/tree/main/Coroutine/all 
2026/5/19
修改内容:
基于上午完成的进程、线程和协程爬虫程序的进程和线程总耗时时常过长、吞吐率过低和进程的内存峰值过高等问题做出一些性能优化:
首先是将main函数改为无async,通过runtime::block_on阻塞后再调用协程检测,同时修改corout.rs,将其爬取网址内容fetch_data和写入文件操作write_school_data进行分离,通过mpsc::channel 将多个爬取数据传给唯一的写入端,并通过oneshot::channel返回通知,以此确保依次写入,另外还有一些较小改动,不再赘述
运行结果:
https://github.com/xianxw/Asyncos/blob/main/Coroutine/all/reports/second_result.md 
完整代码:
https://github.com/xianxw/Asyncos/tree/main/Coroutine/all 
