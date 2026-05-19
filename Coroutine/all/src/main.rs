mod schools;
mod neicun;
mod crawler;

use crawler::BenchmarkResult;
use crawler::corout;
use crawler::process;
use crawler::thread;
use schools::load_targets;
use std::env;

fn print_comparison(results: &[&BenchmarkResult]) {
	println!("\n=== 三种爬虫性能对比 ===");
	println!("{:<12} {:>8} {:>8} {:>10} {:>12} {:>12} {:>12}", "类型", "总数", "成功", "耗时(s)", "吞吐", "P50(ms)", "内存(KB)");

	for result in results {
		println!(
			"{:<12} {:>8} {:>8} {:>10.2} {:>12.2} {:>12.2} {:>12}",
			result.model_name,
			result.total_requests,
			result.success_requests,
			result.total_time_secs,
			result.throughput,
			result.latency_p50,
			result.memory_peak_kb,
		);
	}
}


fn main() {
	let args: Vec<String> = env::args().collect();
	if args.len() > 1 && args[1] == "--worker" {
		let _ = process::process(&[]);
		return;
	}

	let schools = load_targets().unwrap();

	let thread_result = thread::thread(&schools);
	let process_result = process::process(&schools);
	let runtime = tokio::runtime::Runtime::new().unwrap();
	let corout_result = runtime.block_on(corout::corout(&schools));

	thread_result.print_report();
	process_result.print_report();
	corout_result.print_report();

	print_comparison(&[&thread_result, &process_result, &corout_result]);
}
