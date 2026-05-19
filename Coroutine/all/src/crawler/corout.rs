use crate::schools::School;
use crate::neicun::measure_async_peak_kb;
use reqwest::Client;
use scraper::{Html, Selector};
use std::path::Path;
use std::time::{Duration, Instant};
use super::BenchmarkResult;
use tokio::fs;
use tokio::fs::File as AsyncFile;
use tokio::io::AsyncWriteExt;

#[derive(Debug)]
struct FetchMetric {
    success: bool,
    latency_ms: f64,
}

async fn fetch_data(client: Client, url: String, name: String) -> FetchMetric {
    let started = Instant::now();

    let result: Result<(), String> = async {
        let response = client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|err| err.to_string())?;

        let response_text = response.text().await.map_err(|err| err.to_string())?;

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

        let output_dir = "school_data";
        fs::create_dir_all(output_dir)
            .await
            .map_err(|err| err.to_string())?;

        let file_path = Path::new(output_dir).join(format!("{}.txt", name));
        let mut file = AsyncFile::create(&file_path)
            .await
            .map_err(|err| err.to_string())?;
        file.write_all(seek_data.as_bytes())
            .await
            .map_err(|err| err.to_string())?;

        Ok(())
    }
    .await;

    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    FetchMetric {
        success: result.is_ok(),
        latency_ms,
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

pub async fn corout(schools: &[School]) -> BenchmarkResult {
    let (result, memory_peak_kb) = measure_async_peak_kb(async {
        let start = Instant::now();
        let client = Client::new();

        let mut tasks = Vec::with_capacity(schools.len());
        for school in schools.iter().cloned() {
            let client_clone = client.clone();
            tasks.push(tokio::spawn(async move {
                fetch_data(client_clone, school.url, school.name).await
            }));
        }

        let mut metrics = Vec::with_capacity(schools.len());
        for task in tasks {
            if let Ok(metric) = task.await {
                metrics.push(metric);
            }
        }

        let total_time_secs = start.elapsed().as_secs_f64();
        let success_requests = metrics.iter().filter(|metric| metric.success).count();
        let latencies: Vec<f64> = metrics.iter().map(|metric| metric.latency_ms).collect();
        let throughput = if total_time_secs > 0.0 {
            success_requests as f64 / total_time_secs
        } else {
            0.0
        };

        BenchmarkResult {
            model_name: "协程爬虫".to_string(),
            total_requests: schools.len(),
            success_requests,
            total_time_secs,
            throughput,
            latency_p50: percentile(&{
                let mut sorted = latencies.clone();
                sorted.sort_by(|left, right| left.partial_cmp(right).unwrap());
                sorted
            }, 0.50),
            latency_p95: percentile(&{
                let mut sorted = latencies.clone();
                sorted.sort_by(|left, right| left.partial_cmp(right).unwrap());
                sorted
            }, 0.95),
            memory_peak_kb: 0,
        }
    }).await;

    BenchmarkResult {
        memory_peak_kb,
        ..result
    }
}