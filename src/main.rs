use chrono::{DateTime, Utc};
use livescores::{GamesPage, MainPage};
use reqwest::{Client, StatusCode};
use scraper::Html;
use std::collections::HashMap;
use std::{io::Write, vec};
use tokio::time::sleep;

mod livescores;

use crate::livescores::LeagueGroupPage;

static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
pub fn get_client() -> Client {
    let client = Client::builder()
        .user_agent(APP_USER_AGENT)
        .build()
        .unwrap();
    client
}

#[derive(Debug)]
pub struct ScraperTask {
    url: String,
    href: String,
    name: String,
}

pub trait ScripingFunction {
    fn name(&self) -> &'static str;
    fn parse(&self, task: &ScraperTask, document: &Html) -> Result<ScraperTaskResult, TaskError>;
    fn new_task(url: &str, href: &str) -> ScraperTask
    where
        Self: Sized;
}

pub struct Scraper {
    client: Client,
    parsers: HashMap<String, Box<dyn ScripingFunction>>,
}

#[derive(Debug)]
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
    Fragment(String, String),
    NoParsingFunction(String),
}

impl Scraper {
    async fn scripe(&self, task: &ScraperTask) -> Result<ScraperTaskResult, TaskError> {
        let url = format!("{}{}", &task.url, &task.href);
        let result = self.client.get(&url).send().await;
        match result {
            Ok(response) => match response.status() {
                StatusCode::OK => {
                    let raw_html = response.text().await.unwrap();
                    let document = Html::parse_document(&raw_html);
                    if let Some(parse) = self.parsers.get(&task.name) {
                        parse.parse(task, &document)
                    } else {
                        Err(TaskError::NoParsingFunction(format!(
                            "No parsing function {}",
                            &task.name
                        )))
                    }
                }
                _ => Err(TaskError::Other("Something went wrong".to_owned())),
            },
            Err(e) => Err(TaskError::Other(e.to_string())),
        }
    }

    fn add_scriping_function(&mut self, f: Box<dyn ScripingFunction>) {
        self.parsers.insert(f.name().to_owned(), f);
    }
}

#[tokio::main]
async fn main() {
    let mut queue: Vec<ScraperTask> = vec![];
    queue.push(MainPage::new_task("https://www.livescores.com", ""));
    let mut scraper = Scraper {
        client: get_client(),
        parsers: HashMap::new(),
    };
    scraper.add_scriping_function(Box::new(MainPage::default()));
    scraper.add_scriping_function(Box::new(LeagueGroupPage::default()));
    scraper.add_scriping_function(Box::new(GamesPage::default()));

    while queue.len() > 0 {
        if let Some(task) = queue.pop() {
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
                }
                Err(TaskError::Parsing(document)) => {
                    let filename = format!("{}.html", task.name);
                    println!("Writting {}", &filename);
                    let content = document.root_element().html();
                    let mut file = std::fs::File::create(&filename).expect("create failed");
                    file.write_all(content.as_bytes()).expect("write failed");
                }
                Err(TaskError::Fragment(name, inner_html)) => {
                    println!("Error parsing framgent {}\n{}\n", name, inner_html);
                }
                Err(TaskError::NoParsingFunction(taks_name)) => {
                    println!("No parsing function for task {}", taks_name);
                }
                Err(TaskError::Other(message)) => {
                    println!("Other Error {}", message)
                }
            };
            sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}
