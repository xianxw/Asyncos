use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::time::Duration;
use std::thread; 

#[derive(Debug)]
struct School {
    name: String,
    url: String,
}

fn load_targets() -> io::Result<Vec<School>> {
    let file = File::open("school.txt")?;
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

fn fetch_data(name: &str, url: &str, client: Client) {

    let response = match client.get(url).send() {
        Ok(res) => res,
        Err(e) => {
            eprintln!("❌ [线程] {} 网络请求失败: {}", name, e);
            return;
        }
    };
    
    let response_text = match response.text() {
        Ok(txt) => txt,
        Err(_) => return,
    };

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
        };
    };
    let output = "school_data_thread"; 
    fs::create_dir_all(output).unwrap();
    
    let file_name = format!("{}.txt", name);
    let file_path = Path::new(output).join(file_name);
    
    let mut file = File::create(&file_path).unwrap();
    file.write_all(seek_data.as_bytes()).unwrap();
    
    println!("✅ [线程] {} 抓取完毕！", name);
}

fn main() {
    let client = Client::new();
        
    let schools = load_targets().unwrap();
    
    let mut handles = vec![];

    println!("🚀 [主线程] 开始并发派发任务，共 {} 个...", schools.len());

    for school in schools {
        let cli = client.clone();
        

        let handle = thread::spawn(move || {

            fetch_data(&school.name, &school.url, cli);
        });
        
        handles.push(handle);
    }
    
    println!("⏳ [主线程] 任务全部分配完毕，等待子线程收工...");

    for handle in handles {
        let _ = handle.join();
    }
    
    println!("🎉 [主线程] 所有打工人收工，程序完美退出！");
}