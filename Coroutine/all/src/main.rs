mod schools;
mod neicun;
mod crawler;

use crawler::BenchmarkResult;
use crawler::corout;
use crawler::process;
use crawler::thread;
use schools::load_targets;

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

#[tokio::main]
async fn main() {
	let schools = match load_targets() {
		Ok(schools) => schools,
		Err(err) => {
			eprintln!("读取 school.txt 失败: {}", err);
			return;
		}
	};

	let thread_result = thread::thread(&schools);
	let process_result = process::process(&schools);
	let corout_result = corout::corout(&schools).await;

	thread_result.print_report();
	process_result.print_report();
	corout_result.print_report();

	print_comparison(&[&thread_result, &process_result, &corout_result]);
}
