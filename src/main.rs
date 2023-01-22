use chrono::{DateTime, Utc};
use reqwest::{StatusCode, Client};
use scraper::{Html};
use tokio::time::sleep;
use std::{io::Write, vec};
use std::collections::HashMap;

mod livescores;
use crate::livescores::parse;
use crate::livescores::parse_leagues;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
pub fn get_client() -> Client {
    let client = Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()
        .unwrap();
    client
}



pub struct Team {
    pub name: String,
    pub country: String,

}

pub struct Match {
    pub league: String,
    pub start: DateTime<chrono::Utc>,
    pub host: Team,
    pub guest: Team,
}


pub struct Scraper {
    client: Client,
    parse_fn: HashMap<String, &'static dyn Fn(&ScraperTask, &Html) -> Result<ScraperTaskResult, TaskError>>,   
}

pub struct ScraperTask {
    url: String,
    href: String,
    name: String,
}

pub struct ScraperTaskResult {
    url: String,
    data: String,
    success: bool,
    last_update: DateTime<chrono::Utc>,
    new_urls: Vec<ScraperTask>,
}


#[derive(Debug)]
pub enum TaskError {
    Other(String),
    Parsing(Html),
    NoParsingFunction(String),
}


trait ScipingFunction {
    fn name() -> String;
    fn parse(task: &ScraperTask, document: &Html) -> Result<ScraperTaskResult, TaskError>;
}

impl Scraper {
    async fn scripe(&self, task: &ScraperTask) -> Result<ScraperTaskResult, TaskError> {
        let url = format!("{}{}", &task.url, &task.href);
        let result = self.client.get(&url).send().await;
        match result {
            Ok(response) => {
                match response.status() {
                    StatusCode::OK => {
                        let raw_html = response.text().await.unwrap();
                        let document = Html::parse_document(&raw_html);
                        if let Some(parse) = self.parse_fn.get(&task.name) {
                            (parse)(task, &document)
                        } else {
                            Err(TaskError::NoParsingFunction(format!("No parsing function {}", &task.name)))
                        }
                    },
                    _ => {
                        Err(TaskError::Other("Something went wrong".to_owned()))
                    },
                }
            },
            Err(e) => {
                Err(TaskError::Other(e.to_string()))            }
        }
    }

    fn add_parse(&mut self, name: String, func: &'static dyn Fn(&ScraperTask, &Html) -> Result<ScraperTaskResult, TaskError>) {
        self.parse_fn.insert(name, func);
    }

}




#[tokio::main]
async fn main() {

    let mut queue: Vec<ScraperTask> = vec![];
    queue.push(ScraperTask {
        url: "https://www.livescores.com".to_owned(),
        href: "/".to_owned(),
        name: "parse".to_owned(),
    });

    let mut scraper = Scraper { client: get_client(), parse_fn: HashMap::new()};
    scraper.add_parse("parse".to_owned(),  &parse);
    scraper.add_parse("parse_country".to_owned(), &parse_leagues);
    
    while queue.len() > 0 {
        if let Some(task) = queue.pop() {
            let filename = format!("{}.html", task.name);
            println!("Scriping {}", task.url);
            if std::path::Path::new(&filename).exists() {
                println!("Skipping {} because it already exists", &filename);
                let res = scraper.scripe(&task).await;
                match res {
                    Ok(mut result) => {
                        println!("Data = {}", result.data);
                        println!("Last update = {}", result.last_update);
                        println!("Success = {}", result.success);
                        if result.success {
                            queue.append(&mut result.new_urls);
                        } else {
                            // redo later
                        }
                    },
                    Err(TaskError::Parsing(document)) => {
                        let content = document.root_element().html();
                        let mut file = std::fs::File::create(&filename).expect("create failed");
                        file.write_all(content.as_bytes()).expect("write failed");
                    },
                    Err(TaskError::NoParsingFunction(taks_name)) => {
                        println!("No parsing function for task {}", taks_name);
                    }, 
                    Err(TaskError::Other(message)) => {
                        println!("Other Error {}", message)
                    }
                };
            };
            sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}