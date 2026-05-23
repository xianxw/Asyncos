use std::fs::File as StdFile;
use std::io::{self, BufRead, BufReader};

#[derive(Debug, Clone)]
pub struct School {
    pub name: String,
    pub url: String,
}

pub fn load_targets() -> io::Result<Vec<School>> {
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