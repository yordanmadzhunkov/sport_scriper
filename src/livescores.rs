use std::vec;

use crate::{ScraperTask, ScraperTaskResult, TaskError};
use scraper::{Html, Selector};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};



fn should_follow(title: &String) -> bool {
    let exclude_list = ["Home", "Live", "Favourites"].map(|s| s.to_string());
    for item in exclude_list {
        if item.eq(title) {
            return false;
        }
    }
    true
}


#[derive(Serialize, Deserialize)]
pub struct League {
    name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Game {

}


#[derive(Serialize, Deserialize)]
pub struct Data {
    leagues: Vec<League>,
    games: Vec<Game>,
}

pub fn parse_leagues(task: &ScraperTask, document: &Html) -> Result<ScraperTaskResult, TaskError> {
    let mut new_tasks: Vec<ScraperTask> = vec![];
    let league_selector = Selector::parse(".se li > ul > li > a").unwrap();

    let mut data = Data {
        leagues: vec![],
        games: vec![],
    };
    for element in document.select(&league_selector) {
        match element.value().attr("href") {
            Some(href) => {
                let title = element.text().collect::<String>();
                println!("{}", title);
                data.leagues.push(League{name: title});
                new_tasks.push(
                    ScraperTask{url: task.url.clone(), href: href.to_string(), name: "parse_country_league".to_owned()}
                );
            },
            None => {
                return Err(TaskError::Parsing(document.clone()));
            } 
        };
    }
    
    Ok(ScraperTaskResult {
        url: task.url.clone(),
        data: serde_json::to_string(&data).unwrap(),
        success: true,
        last_update: Utc::now(),
        new_urls: new_tasks,
    })
}




pub fn parse(task: &ScraperTask, document: &Html) -> Result<ScraperTaskResult, TaskError> {
    let mut new_tasks: Vec<ScraperTask> = vec![];

    let league_selector = Selector::parse("a.ue").unwrap();
    for element in document.select(&league_selector) {
        match element.value().attr("href") {
            Some(href) => {
                let title = element.text().collect::<String>();
                if should_follow(&title) {
                    //let target = format!("{}{}", &task.url, &href);
                    new_tasks.push(
                        ScraperTask{url: task.url.clone(), href: href.to_string(), name: "parse_country".to_owned()}
                    );
                }
            },
            None => {
                return Err(TaskError::Parsing(document.clone()));
            } 
        };
    }

    Ok(ScraperTaskResult {
        url: task.url.clone(),
        data: "".to_owned(),
        success: true,
        last_update: Utc::now(),
        new_urls: new_tasks,
    })
}

#[cfg(test)]
mod tests {
    use scraper::Html;

    use crate::{ScraperTask, ScraperTaskResult, TaskError};
    use crate::livescores::parse_leagues;
    use crate::livescores::Data;

    #[test]
    fn test_parse_county_page() {
        let filename = "test_data/parse_country.html";
        let content = std::fs::read_to_string(&filename).expect("cant read file");
        let document = Html::parse_document(&content);
        let task = ScraperTask {
            url: "livescores.com".to_owned(),
            href: "".to_owned(),
            name: "noname".to_owned(),
        };
        let p = parse_leagues(&task, &document).expect("no error expected");
        let data: Data = serde_json::from_str(&p.data).expect("parsing error from serde json");
        assert_eq!(data.leagues.len(), 14);
    }
}