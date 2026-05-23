use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write}; 
use std::path::Path;
use std::time::Duration;
use std::env;
use std::process::{Command, Child}; 

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

fn fetch_data(name: &str, url: &str) {
    let client = Client::new();

    let response = client.get(url).send().unwrap();
    

    let response_text = response.text().unwrap(); 
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

    let output = "school_data_process";

    
    let file_name = format!("{}.txt", name);
    let file_path = Path::new(output).join(file_name);
    
    let mut file = File::create(&file_path).unwrap();
    file.write_all(seek_data.as_bytes()).unwrap();
    
    println!("✅ [子进程] {} 抓取完毕！", name);
}

fn main() {
    //获取命令行参数,通过命令行的参数标记子进程,并通过命令行参数让子进程获取所需的name和url
    let args: Vec<String> = env::args().collect();


    if args.len() > 1 && args[1] == "--worker" {
        let name = &args[2];
        let url = &args[3];
        fetch_data(name, url); 
        std::process::exit(0); 
    }

    let schools = load_targets().unwrap();
    let mut child_handles: Vec<Child> = Vec::new();
    let current_exe = env::current_exe().unwrap();

    println!("🚀 [主进程] 开始拉起并发子进程，共 {} 个任务...", schools.len());

    for school in schools {
        let child = Command::new(&current_exe)
            .arg("--worker")
            .arg(school.name)
            .arg(school.url)
            .spawn()
            .expect("拉起子进程失败");
            
        child_handles.push(child);
    }
    
    // 等待所有子进程
    for mut child in child_handles {
        let status = child.wait().expect("等待子进程时出错");
        if !status.success() {
            eprintln!("⚠️ 某个子进程异常退出");
        }
    }
    
    println!("🎉 [主进程] 所有爬虫任务执行完毕！");
}