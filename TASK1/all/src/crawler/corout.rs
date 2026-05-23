use crate::schools::School;
use crate::neicun::measure_async_peak_kb;
use reqwest::Client;
use scraper::{Html, Selector};
use std::path::Path;
use std::time::Instant;
use super::{benchmark_concurrency_limit, percentile, BenchmarkResult};
use tokio::fs;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug)]
struct FetchMetric {
    success: bool,
    latency_ms: f64,
}

struct WriteJob {
    name: String,
    seek_data: String,
    completion: oneshot::Sender<Result<(), String>>,
}

async fn write_school_data(
    mut receiver: mpsc::Receiver<WriteJob>,
    output_dir: &'static str,
) {
    while let Some(job) = receiver.recv().await {
        let file_path = Path::new(output_dir).join(format!("{}.txt", job.name));
        let result = fs::write(&file_path, job.seek_data.as_bytes())
            .await
            .map_err(|err| err.to_string());
        let _ = job.completion.send(result);
    }
}

async fn fetch_data(
    client: Client,
    url: String,
    name: String,
    writer: mpsc::Sender<WriteJob>,
) -> FetchMetric {
    let started = Instant::now();

    let result: Result<(), String> = async {
        let response = client
            .get(&url)
            .send()
            .await
            .unwrap();

        let response_text = response.text().await.unwrap();

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

        let (completion_tx, completion_rx) = oneshot::channel();
        writer
            .send(WriteJob {
                name,
                seek_data,
                completion: completion_tx,
            })
            .await
            .map_err(|err| err.to_string())?;

        let write_result = completion_rx.await.map_err(|err| err.to_string())?;
        write_result?;

        Ok(())
    }
    .await;

    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    FetchMetric {
        success: result.is_ok(),
        latency_ms,
    }
}

pub async fn corout(schools: &[School]) -> BenchmarkResult {
    let (result, memory_peak_kb) = measure_async_peak_kb(async {
        let start = Instant::now();
        let client = Client::new();
        let total_requests = schools.len();
        let concurrency = benchmark_concurrency_limit(total_requests);
        let output_dir = "school_data";

        let _ = fs::create_dir_all(output_dir).await;
        let (writer_tx, writer_rx) = mpsc::channel::<WriteJob>(concurrency.max(1));
        let writer_handle = tokio::spawn(write_school_data(writer_rx, output_dir));

        let mut metrics = Vec::with_capacity(total_requests);
        for batch in schools.chunks(concurrency) {
            let mut tasks = Vec::with_capacity(batch.len());
            for school in batch.iter().cloned() {
                let client_clone = client.clone();
                let writer_tx = writer_tx.clone();
                tasks.push(tokio::spawn(async move {
                    fetch_data(client_clone, school.url, school.name, writer_tx).await
                }));
            }

            for task in tasks {
                if let Ok(metric) = task.await {
                    metrics.push(metric);
                }
            }
        }

        drop(writer_tx);
        writer_handle
            .await
            .expect("异步写文件任务发生 panic");

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
            total_requests,
            success_requests,
            total_time_secs,
            throughput,
            latency_p50: percentile(&latencies, 0.50),
            latency_p95: percentile(&latencies, 0.95),
            memory_peak_kb: 0,
        }
    }).await;

    BenchmarkResult {
        memory_peak_kb,
        ..result
    }
}