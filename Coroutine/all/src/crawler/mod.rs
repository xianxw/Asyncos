pub mod corout;
pub mod process;
pub mod thread;
#[derive(Debug)]
pub struct BenchmarkResult {
    pub model_name: String,       
    pub total_requests: usize,   
    pub success_requests: usize,  
    pub total_time_secs: f64,     
    pub throughput: f64,         
    pub latency_p50: f64,         
    pub latency_p95: f64,      
    pub memory_peak_kb: u64,      
}

impl BenchmarkResult {

    pub fn print_report(&self) {
        println!("\n=== {} 性能测试报告 ===", self.model_name);
        println!("总请求数:   {}", self.total_requests);
        println!("成功请求数: {}", self.success_requests);
        println!("总耗时:     {:.2} 秒", self.total_time_secs);
        println!("吞吐率:     {:.2} req/s", self.throughput);
        println!("延迟 P50:   {:.2} ms", self.latency_p50);
        println!("延迟 P95:   {:.2} ms", self.latency_p95);
        println!("内存峰值:   {} KB ({:.2} MB)", self.memory_peak_kb, self.memory_peak_kb as f64 / 1024.0);
        println!("===============================\n");
    }
}