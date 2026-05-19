use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;
use sysinfo::{Pid, Process, System};

/// 采样当前进程的峰值内存占用
pub struct MemoryMonitor {
    is_running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<u64>>,
}

impl MemoryMonitor {
    /// 启动后台采样线程
    pub fn start() -> Self {
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = is_running.clone();
        let pid = sysinfo::get_current_pid().expect("无法获取 PID");

        let handle = thread::spawn(move || {
            let mut sys = System::new_all();
            let mut max_mem: u64 = 0;
            while is_running_clone.load(Ordering::Relaxed) {
                sys.refresh_all();
                let mem = process_tree_memory_kb(&sys, pid);
                if mem > max_mem {
                    max_mem = mem;
                }
                thread::sleep(Duration::from_millis(50));
            }
            max_mem
        });

        Self {
            is_running,
            handle: Some(handle),
        }
    }

    /// 停止采样并返回峰值内存
    pub fn stop(mut self) -> u64 {
        self.is_running.store(false, Ordering::Relaxed);
        self.handle.take().unwrap().join().unwrap()
    }
}

fn is_descendant_of(pid: Pid, root_pid: Pid, processes: &HashMap<Pid, Process>) -> bool {
    let mut current_pid = pid;

    loop {
        if current_pid == root_pid {
            return true;
        }

        let Some(process) = processes.get(&current_pid) else {
            return false;
        };

        let Some(parent_pid) = process.parent() else {
            return false;
        };

        current_pid = parent_pid;
    }
}

fn process_tree_memory_kb(system: &System, root_pid: Pid) -> u64 {
    system
        .processes()
        .iter()
        .filter(|(pid, _)| is_descendant_of(**pid, root_pid, system.processes()))
        .map(|(_, process)| process.memory() / 1024)
        .sum()
}

/// 包裹一个同步执行体，返回执行结果和峰值内存
pub fn measure_sync_peak_kb<F, T>(f: F) -> (T, u64)
where
    F: FnOnce() -> T,
{
    let monitor = MemoryMonitor::start();
    let result = f();
    let peak_kb = monitor.stop();
    (result, peak_kb)
}

/// 包裹一个异步执行体，返回执行结果和峰值内存
pub async fn measure_async_peak_kb<F, T>(future: F) -> (T, u64)
where
    F: Future<Output = T>,
{
    let monitor = MemoryMonitor::start();
    let result = future.await;
    let peak_kb = monitor.stop();
    (result, peak_kb)
}