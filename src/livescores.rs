use std::vec;

use crate::{ScraperTask, ScraperTaskResult, TaskError};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

use crate::ScripingFunction;

#[derive(Serialize, Deserialize)]
pub struct League {
    name: String,
}

#[derive(Serialize, Deserialize, Default)]
pub struct LeagueGroupPage {
    pub leagues: Vec<League>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct MainPage {}

#[derive(Serialize, Deserialize, Default)]
pub struct GamesPage {
    pub games: Vec<Game>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Team {
    pub name: String,
    pub country: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum MatchStatus {
    Scheduled(NaiveTime),
    Postponed,
    InPlay(i32, i32),
    Finished(i32, i32),
    //Maybe other interupted ?
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Game {
    pub status: MatchStatus,
    pub league: String,
    pub stage: Option<String>,
    pub start_date: NaiveDate,
    pub host: Team,
    pub guest: Team,
}

impl ScripingFunction for LeagueGroupPage {
    fn name(&self) -> &'static str {
        Self::my_name()
    }

    fn new_task(url: &str, href: &str) -> ScraperTask {
        ScraperTask {
            url: url.to_string(),
            href: href.to_string(),
            name: Self::my_name().to_owned(),
        }
    }

    fn parse(&self, task: &ScraperTask, document: &Html) -> Result<ScraperTaskResult, TaskError> {
        let mut new_tasks: Vec<ScraperTask> = vec![];
        let league_selector = Selector::parse(".se li > ul > li > a").unwrap();

        let mut data = LeagueGroupPage::default();
        for element in document.select(&league_selector) {
            match element.value().attr("href") {
                Some(href) => {
                    let title = element.text().collect::<String>();
                    data.leagues.push(League { name: title });
                    new_tasks.push(ScraperTask {
                        url: task.url.clone(),
                        href: href.to_string(),
                        name: GamesPage::my_name().to_owned(),
                    });
                }
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
}

impl MainPage {
    pub fn my_name() -> &'static str {
        "main"
    }
}
impl LeagueGroupPage {
    pub fn my_name() -> &'static str {
        "league_group"
    }
}

impl GamesPage {
    pub fn my_name() -> &'static str {
        "games"
    }
}

impl ScripingFunction for MainPage {
    fn name(&self) -> &'static str {
        Self::my_name()
    }

    fn new_task(url: &str, href: &str) -> ScraperTask {
        ScraperTask {
            url: url.to_string(),
            href: href.to_string(),
            name: Self::my_name().to_owned(),
        }
    }

    fn parse(&self, task: &ScraperTask, document: &Html) -> Result<ScraperTaskResult, TaskError> {
        let mut new_tasks: Vec<ScraperTask> = vec![];
        let league_selector = Selector::parse("a.ue").unwrap();
        for element in document.select(&league_selector) {
            match element.value().attr("href") {
                Some(href) => {
                    let title = element.text().collect::<String>();
                    if Self::should_follow(&title) {
                        new_tasks.push(LeagueGroupPage::new_task(&task.url, href));
                    }
                }
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
}

impl ScripingFunction for GamesPage {
    fn name(&self) -> &'static str {
        Self::my_name()
    }

    fn new_task(url: &str, href: &str) -> ScraperTask {
        ScraperTask {
            url: url.to_string(),
            href: href.to_string(),
            name: Self::my_name().to_owned(),
        }
    }

    fn parse(&self, task: &ScraperTask, document: &Html) -> Result<ScraperTaskResult, TaskError> {
        let selector = Selector::parse("div.xb > div.bb, div.xf").unwrap();
        let select_date = Selector::parse("span.cb").unwrap();
        let select_game = Selector::parse("a.qd").unwrap();
        let select_stage = Selector::parse("span.fb").unwrap();
        let select_league = Selector::parse("span.eb").unwrap();

        let mut data = GamesPage::default();
        let mut date_header: Option<NaiveDate> = None;
        let mut league: String = "".to_string();
        let mut league_stage: Option<String> = None;
        for element in document.select(&selector) {
            for d in element.select(&select_date) {
                let date_str = d.text().collect::<String>();
                date_header = parse_date(&date_str, 2023);
            }
            for game_element in element.select(&select_game) {
                if let Some(league_element) = game_element.select(&select_league).nth(0) {
                    league_stage = None;
                    league = league_element.text().collect::<String>();
                } else if let Some(league_element) = game_element.select(&select_stage).nth(0) {
                    league_stage = Some(league_element.text().collect::<String>());
                } else if let Some(start_date) = date_header {
                    data.games.push(self.parse_game(
                        game_element,
                        &league,
                        &league_stage,
                        start_date,
                    )?);
                } else {
                    return Err(TaskError::Fragment(
                        "game element".to_string(),
                        game_element.html(),
                    ));
                }
            }
        }
        Ok(ScraperTaskResult {
            url: task.url.clone(),
            data: serde_json::to_string(&data).unwrap(),
            success: true,
            last_update: Utc::now(),
            new_urls: vec![],
        })
    }
}

impl GamesPage {
    fn parse_score(&self, game_element: ElementRef) -> Result<(i32, i32), TaskError> {
        let select_score_home = Selector::parse("span.hh").unwrap();
        let select_score_guest = Selector::parse("span.ih").unwrap();
        let mut home = -1;
        let mut away = -1;
        if let Some(home_score) = game_element.select(&select_score_home).nth(0) {
            home = home_score.text().collect::<String>().parse().unwrap_or(-1);
        }
        if let Some(guest_score) = game_element.select(&select_score_guest).nth(0) {
            away = guest_score.text().collect::<String>().parse().unwrap_or(-1);
        }
        if home < 0 || away < 0 {
            Err(TaskError::Fragment(
                "Parse game score".to_owned(),
                game_element.inner_html(),
            ))
        } else {
            Ok((home, away))
        }
    }

    fn parse_teams(&self, game_element: ElementRef) -> Result<(String, String), TaskError> {
        let select_teams = Selector::parse("span.eh").unwrap();
        if let Some(team) = game_element.select(&select_teams).nth(0) {
            let home_team = team.text().collect::<String>();
            if let Some(team) = game_element.select(&select_teams).nth(1) {
                let away_team = team.text().collect::<String>();
                Ok((home_team, away_team))
            } else {
                Err(TaskError::Fragment(
                    "Parse game away team".to_owned(),
                    game_element.inner_html(),
                ))
            }
        } else {
            Err(TaskError::Fragment(
                "Parse game home team".to_owned(),
                game_element.inner_html(),
            ))
        }
    }

    fn parse_game_status(&self, game_element: ElementRef) -> Result<MatchStatus, TaskError> {
        let select_start_time = Selector::parse("span.Pg").unwrap();
        if let Some(start_time_element) = game_element.select(&select_start_time).nth(0) {
            let c = start_time_element.text().collect::<String>();
            if c == "FT".to_string() || c == "AET".to_string() || c == "AAW".to_string() {
                let score = self.parse_score(game_element)?;
                Ok(MatchStatus::Finished(score.0, score.1))
            } else {
                if let Ok(tt) = chrono::NaiveTime::parse_from_str(&c, "%H:%M") {
                    Ok(MatchStatus::Scheduled(tt))
                } else {
                    Err(TaskError::Fragment(
                        "Parsing game status".to_string(),
                        game_element.html(),
                    ))
                }
            }
        } else {
            Err(TaskError::Fragment(
                "Selector select_start_time failed".to_string(),
                game_element.html(),
            ))
        }
    }

    fn parse_game(
        &self,
        game_element: ElementRef,
        league: &String,
        league_stage: &Option<String>,
        start_date: NaiveDate,
    ) -> Result<Game, TaskError> {
        let status = self.parse_game_status(game_element)?;
        let (home_team, away_team) = self.parse_teams(game_element)?;
        let game = Game {
            status,
            league: league.clone(),
            stage: league_stage.clone(),
            start_date,
            host: Team {
                name: home_team,
                country: "".into(),
            },
            guest: Team {
                name: away_team,
                country: "".into(),
            },
        };
        Ok(game)
    }
}

fn parse_date(date_str: &str, default_year: i32) -> Option<NaiveDate> {
    let mut day: u32 = 0;
    let mut month: u32 = 0;
    let mut year = default_year;
    for el in date_str.split([' ', ',']).enumerate() {
        if el.0 == 1 {
            day = el.1.parse::<u32>().unwrap_or(0);
        } else if el.0 == 0 {
            let months = [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ];
            for i in 0..12 {
                if el.1 == months[i] {
                    month = (i + 1) as u32;
                    break;
                }
            }
        } else if el.0 == 3 {
            year = el.1.parse::<i32>().unwrap_or(default_year);
        }
    }
    NaiveDate::from_ymd_opt(year, month, day)
}

impl MainPage {
    fn should_follow(title: &String) -> bool {
        let exclude_list = ["Home", "Live", "Favourites"].map(|s| s.to_string());
        for item in exclude_list {
            if item.eq(title) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use scraper::Html;

    use crate::livescores::{parse_date, GamesPage, LeagueGroupPage, MatchStatus};
    use crate::ScripingFunction;
    use crate::{ScraperTask, ScraperTaskResult, TaskError};
    use chrono::NaiveDate;

    #[test]
    fn test_parse_league_group_page() {
        let filename = "test_data/parse_country.html";
        let content = std::fs::read_to_string(&filename).expect("cant read file");
        let document = Html::parse_document(&content);
        let league_group: LeagueGroupPage = Default::default();
        let task = LeagueGroupPage::new_task("https://livescores.com", "/");
        let p = league_group
            .parse(&task, &document)
            .expect("Parsing error :(");
        let data: LeagueGroupPage =
            serde_json::from_str(&p.data).expect("parsing error from serde json");
        assert_eq!(data.leagues.len(), 14);
        assert_eq!(
            data.leagues[0].name,
            "Inter-Confederation Qualification: Play-off"
        );

        assert_eq!(p.new_urls.len(), 14);
        assert_eq!(p.new_urls[0].name, "games");
        assert_eq!(p.new_urls[0].url, "https://livescores.com");
        assert_eq!(
            p.new_urls[0].href,
            "/football/womens-world-cup-qualification/inter-confederation-qualification-play-off/"
        );
    }

    #[test]
    fn test_parse_games_page() {
        let filename = "test_data/parse_country.html";
        let content = std::fs::read_to_string(&filename).expect("cant read file");
        let document = Html::parse_document(&content);
        let games_page: GamesPage = Default::default();
        let task = GamesPage::new_task("https://livescores.com", "/");
        let p = games_page
            .parse(&task, &document)
            .expect("Parsing error :(");
        let data: GamesPage = serde_json::from_str(&p.data).expect("parsing error from serde json");
        assert_eq!(data.games.len(), 53);
        let mut i = 0;
        for game in &data.games {
            println!(
                "{:3} {:20} {:13?} {:10?} {} {} {}",
                i,
                game.league,
                game.stage,
                game.status,
                game.start_date,
                game.host.name,
                game.guest.name
            );
            i = i + 1;
        }
        assert_eq!(data.games[36].status, MatchStatus::Finished(0, 8));
        assert_eq!(data.games[36].host.name, "Bulgaria Women".to_owned());
        assert_eq!(data.games[36].guest.name, "Germany Women".to_owned());
        assert_eq!(
            data.games[36].league,
            "Women's World Cup Qualification".to_owned()
        );
        assert_eq!(
            data.games[36].stage,
            Some("UEFA Qualification: Group H".to_string())
        );
    }

    #[test]
    fn test_parse_games_2023_01_28() {
        let filename = "test_data/games_2023_01_28.html";
        let content = std::fs::read_to_string(&filename).expect("cant read file");
        let document = Html::parse_document(&content);
        println!("{:?}", document.errors.len());
        let games_page: GamesPage = Default::default();
        let task = GamesPage::new_task("https://livescores.com", "/");
        let p = games_page
            .parse(&task, &document)
            .expect("Parsing error :(");
        let data: GamesPage = serde_json::from_str(&p.data).expect("parsing error from serde json");
        assert_eq!(data.games.len(), 2);
    }

    #[test]
    fn test_parse_games_page_date_header() {
        assert_eq!(
            parse_date("February 19", 2023).unwrap(),
            NaiveDate::from_ymd_opt(2023, 2, 19).unwrap()
        );
        assert_eq!(
            parse_date("October 11, 2022", 2023).unwrap(),
            NaiveDate::from_ymd_opt(2022, 10, 11).unwrap()
        );
        assert_eq!(
            parse_date("September 6, 2022", 2023).unwrap(),
            NaiveDate::from_ymd_opt(2022, 9, 6).unwrap()
        );
        assert_eq!(
            parse_date("July 12, 2022", 2023).unwrap(),
            NaiveDate::from_ymd_opt(2022, 7, 12).unwrap()
        );
        assert_eq!(
            parse_date("December 10, 2020", 2023).unwrap(),
            NaiveDate::from_ymd_opt(2020, 12, 10).unwrap()
        );
    }

    #[test]
    fn test_parse_game_fragment() {
        let fragment = r#"<a class="qd" href="/football/europa-league-20-21/qualification-preliminary-round/lincoln-red-imps-fc-vs-fc-prishtina/326775/"><div class="Xg"><span class="Kg"><span data-testid="match_row_time-status_or_time_326775" class="Pg Lg">AAW</span></span><span class="bh"><span class="ch"><span data-testid="football_match_row-home_team_326775" class="eh">Lincoln Red Imps FC</span></span><span class="Zg"><span data-testid="football_match_row-home_score_326775" class="hh">3</span><span class="jh"> <!-- -->-<!-- --> </span><span class="ih" data-testid="football_match_row-away_score_326775">0</span></span><span class="dh"><span data-testid="football_match_row-away_team_326775" class="eh">FC Prishtina</span></span></span></div></a>"#;
        let doc = Html::parse_fragment(fragment);
        let g = GamesPage::default();
        let game = g
            .parse_game(
                doc.root_element(),
                &"Europa League".to_string(),
                &None,
                NaiveDate::from_ymd_opt(2020, 8, 22).unwrap(),
            )
            .unwrap();
        assert_eq!(game.status, MatchStatus::Finished(3, 0));
        assert_eq!(game.host.name, "Lincoln Red Imps FC".to_owned());
        assert_eq!(game.guest.name, "FC Prishtina".to_owned());
        assert_eq!(
            game.start_date,
            NaiveDate::from_ymd_opt(2020, 8, 22).unwrap()
        );
    }
    #[test]
    fn test_parse_game_fragment2() {
        let fragment = r#"<a href="/football/europa-league-20-21/group-g-2020-2021/leicester-city-vs-aek-athens/316190/" class="qd"><div class="Xg"><span class="Kg"><span class="Pg Lg" data-testid="match_row_time-status_or_time_316190">FT</span></span><span class="bh"><span class="ch"><span class="eh" data-testid="football_match_row-home_team_316190">Leicester City</span></span><span class="Zg"><span class="hh" data-testid="football_match_row-home_score_316190">2</span><span class="jh"> <!-- -->-<!-- --> </span><span data-testid="football_match_row-away_score_316190" class="ih">0</span></span><span class="dh"><span data-testid="football_match_row-away_team_316190" class="eh">AEK Athens</span></span></span></div></a>"#;
        let doc = Html::parse_fragment(fragment);
        let g = GamesPage::default();
        let game = g
            .parse_game(
                doc.root_element(),
                &"Europa League".to_string(),
                &None,
                NaiveDate::from_ymd_opt(2020, 8, 22).unwrap(),
            )
            .unwrap();
        assert_eq!(game.status, MatchStatus::Finished(2, 0));
        assert_eq!(game.host.name, "Leicester City".to_owned());
        assert_eq!(game.guest.name, "AEK Athens".to_owned());
        assert_eq!(
            game.start_date,
            NaiveDate::from_ymd_opt(2020, 8, 22).unwrap()
        );
    }
}
