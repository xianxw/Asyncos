use std::fs::File as StdFile; 
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

use reqwest::Client;
use scraper::{Html, Selector};
use tokio::fs::{self, File as AsyncFile}; 
use tokio::io::AsyncWriteExt; 

#[derive(Debug, Clone)]
struct School {
    name: String,
    url: String,
}

fn load_targets() -> io::Result<Vec<School>> {
    let file = StdFile::open("school.txt")?; 
    let reader = BufReader::new(file);
    let mut schools = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() { continue; }
        
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            schools.push(School {
                name: parts[0].to_string(),
                url: parts[1].to_string(),
            });
        }
    }
    Ok(schools)
}

async fn fetch_data(client: Client, url: String, name: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

    let response = client.get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
        
    let response_text = response.text().await?;
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
    };


    let output_dir = "school_data"; 
    fs::create_dir_all(output_dir).await?; 

    let file_name = format!("{}.txt", name);
    let file_path = Path::new(output_dir).join(file_name);
    
    let mut file = AsyncFile::create(&file_path).await?;
    file.write_all(seek_data.as_bytes()).await?; 

    println!("成功抓取并保存: {}", name);
    Ok(())
}

#[tokio::main]
async fn main() {
    let schools = load_targets().expect("无法读取 school.txt，请确保文件存在！");
    
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .unwrap();

    let mut tasks = vec![];

    println!("🚀 启动协程并发抓取，共 {} 个任务...", schools.len());

    for school in schools {
        let client_clone = client.clone(); 
        

        let task = tokio::spawn(async move {
            if let Err(e) = fetch_data(client_clone, school.url, school.name.clone()).await {
                eprintln!("抓取 {} 失败: {}", school.name, e);
            }
        });
        
        tasks.push(task);
    }

    // 等待所有协程
    for task in tasks {
        let _ = task.await;
    }
    

}