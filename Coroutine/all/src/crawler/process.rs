use crate::schools::School;
use crate::neicun::measure_sync_peak_kb;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Instant;

use super::BenchmarkResult;

#[derive(Debug)]
struct FetchMetric {
    success: bool,
    latency_ms: f64,
    peak_kb: u64,
}

fn fetch_data(name: &str, url: &str) -> FetchMetric {
    let (metric, peak_kb) = measure_sync_peak_kb(|| {
        let started = Instant::now();
        let client = Client::new();

        let result: Result<(), ()> = (|| {
            let response = client.get(url).send().map_err(|_| ())?;
            let response_text = response.text().map_err(|_| ())?;

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

            let output = "school_data_process";
            fs::create_dir_all(output).map_err(|_| ())?;

            let file_path = Path::new(output).join(format!("{}.txt", name));
            let mut file = File::create(&file_path).map_err(|_| ())?;
            file.write_all(seek_data.as_bytes()).map_err(|_| ())?;

            Ok(())
        })();

        let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
        if result.is_err() {
            return FetchMetric {
                success: false,
                latency_ms,
                peak_kb: 0,
            };
        }

        FetchMetric {
            success: true,
            latency_ms,
            peak_kb: 0,
        }
    });

    FetchMetric {
        peak_kb,
        ..metric
    }
}

fn percentile(sorted: &[f64], ratio: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    let clamped_ratio = ratio.clamp(0.0, 1.0);
    let index = ((sorted.len() - 1) as f64 * clamped_ratio).round() as usize;
    sorted[index]
}

fn parse_metric(stdout: &str) -> Option<FetchMetric> {
    stdout.lines().find_map(|line| {
        let parts: Vec<&str> = line.trim().split('|').collect();
        if parts.len() != 5 || parts[0] != "METRIC" {
            return None;
        }

        let success = parts[2] == "1";
        let latency_ms = parts[3].parse::<f64>().ok()?;
        let peak_kb = parts[4].parse::<u64>().ok()?;
        Some(FetchMetric {
            success,
            latency_ms,
            peak_kb,
        })
    })
}

fn run_worker_process(name: &str, url: &str) -> Result<FetchMetric, String> {
    let current_exe = env::current_exe().map_err(|err| err.to_string())?;
    let output = Command::new(current_exe)
        .arg("--worker")
        .arg(&name)
        .arg(&url)
        .output()
        .map_err(|err| err.to_string())?;

    if !output.status.success() {
        return Err(format!("worker process exited with {}", output.status));
    }

    parse_metric(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| format!("worker for {} did not return metric data", name))
}

pub fn process(schools: &[School]) -> BenchmarkResult {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 && args[1] == "--worker" {
        let name = &args[2];
        let url = &args[3];
        let metric = fetch_data(name, url);
        println!("METRIC|{}|{}|{}|{}", name, if metric.success { 1 } else { 0 }, metric.latency_ms, metric.peak_kb);
        std::process::exit(if metric.success { 0 } else { 1 });
    }

    let (mut result, parent_peak_kb) = measure_sync_peak_kb(|| {
        let start = Instant::now();

        let mut handles = Vec::with_capacity(schools.len());
        for school in schools.iter().cloned() {
            let name = school.name;
            let url = school.url;
            handles.push(thread::spawn(move || run_worker_process(&name, &url)));
        }

        let mut metrics = Vec::new();
        for handle in handles {
            if let Ok(Ok(metric)) = handle.join() {
                metrics.push(metric);
            }
        }

        let total_time_secs = start.elapsed().as_secs_f64();
        let success_requests = metrics.iter().filter(|metric| metric.success).count();
        let latencies: Vec<f64> = metrics.iter().map(|metric| metric.latency_ms).collect();
        let worker_peak_kb = metrics.iter().map(|metric| metric.peak_kb).sum();
        let throughput = if total_time_secs > 0.0 {
            success_requests as f64 / total_time_secs
        } else {
            0.0
        };

        BenchmarkResult {
            model_name: "进程爬虫".to_string(),
            total_requests: metrics.len(),
            success_requests,
            total_time_secs,
            throughput,
            latency_p50: percentile(&latencies, 0.50),
            latency_p95: percentile(&latencies, 0.95),
            memory_peak_kb: worker_peak_kb,
        }
    });

    result.memory_peak_kb = result.memory_peak_kb.saturating_add(parent_peak_kb);
    result
}