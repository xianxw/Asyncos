use crate::schools::School;
use crate::neicun::measure_sync_peak_kb;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::thread;
use std::time::Instant;

use super::{benchmark_concurrency_limit, percentile, BenchmarkResult};

#[derive(Debug)]
struct FetchMetric {
    success: bool,
    latency_ms: f64,
}

fn fetch_data(name: &str, url: &str, client: Client) -> FetchMetric {
    let started = Instant::now();

    let response = match client.get(url).send() {
        Ok(res) => res,
        Err(_) => {
            return FetchMetric {
                success: false,
                latency_ms: started.elapsed().as_secs_f64() * 1000.0,
            };
        }
    };

    let response_text = match response.text() {
        Ok(text) => text,
        Err(_) => {
            return FetchMetric {
                success: false,
                latency_ms: started.elapsed().as_secs_f64() * 1000.0,
            };
        }
    };

    let seek_data = {
        let document_data = Html::parse_document(&response_text);
        let mut seek_data = String::new();
        let body_selector = Selector::parse("body").unwrap();

        if let Some(body) = document_data.select(&body_selector).next() {
            for text in body.text() {
                let result = text.trim();
                if !result.is_empty() {
                    seek_data.push_str(result);
                    seek_data.push('\n');
                }
            }
        }

        seek_data
    };

    let output = "school_data_thread";
    if fs::create_dir_all(output).is_err() {
        return FetchMetric {
            success: false,
            latency_ms: started.elapsed().as_secs_f64() * 1000.0,
        };
    }

    let file_path = Path::new(output).join(format!("{}.txt", name));
    let write_ok = File::create(&file_path)
        .and_then(|mut file| file.write_all(seek_data.as_bytes()))
        .is_ok();

    FetchMetric {
        success: write_ok,
        latency_ms: started.elapsed().as_secs_f64() * 1000.0,
    }
}

pub fn thread(schools: &[School]) -> BenchmarkResult {
    let (result, memory_peak_kb) = measure_sync_peak_kb(|| {
        let client = Client::new();
        let total_requests = schools.len();
        let start_time = Instant::now();
        let concurrency = benchmark_concurrency_limit(total_requests);

        let mut metrics = Vec::with_capacity(total_requests);
        for batch in schools.chunks(concurrency) {
            let mut handles = Vec::with_capacity(batch.len());
            for school in batch.iter().cloned() {
                let cli = client.clone();
                handles.push(thread::spawn(move || fetch_data(&school.name, &school.url, cli)));
            }

            for handle in handles {
                if let Ok(metric) = handle.join() {
                    metrics.push(metric);
                }
            }
        }

        let total_time_secs = start_time.elapsed().as_secs_f64();
        let success_requests = metrics.iter().filter(|metric| metric.success).count();
        let latencies: Vec<f64> = metrics.iter().map(|metric| metric.latency_ms).collect();
        let throughput = if total_time_secs > 0.0 {
            success_requests as f64 / total_time_secs
        } else {
            0.0
        };

        BenchmarkResult {
            model_name: "多线程爬虫".to_string(),
            total_requests,
            success_requests,
            total_time_secs,
            throughput,
            latency_p50: percentile(&latencies, 0.50),
            latency_p95: percentile(&latencies, 0.95),
            memory_peak_kb: 0,
        }
    });

    BenchmarkResult {
        memory_peak_kb,
        ..result
    }
}
