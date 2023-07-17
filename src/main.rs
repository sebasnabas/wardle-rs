use std::{
    collections::HashMap,
    fs::File,
    io::{prelude::*, BufReader},
    iter::zip,
};

use actix_files as fs;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use log::info;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

const GREEN: &str = "🟩";
const YELLOW: &str = "🟨";
const WHITE: &str = "⬜";

const SERVER_DOMAIN: &str = "localhost";
const SERVER_PORT: u16 = 5000;

#[derive(Debug)]
struct AppState {
    guesses: Vec<String>,
    word: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::Builder::new()
        .filter_module(module_path!(), log::LevelFilter::Info)
        .init();
    HttpServer::new(|| {
        App::new()
            .app_data(web::Data::new(AppState {
                guesses: get_guesses(),
                word: get_answers()
                    .choose(&mut rand::thread_rng())
                    .unwrap()
                    .to_string(),
            }))
            .service(game)
            .service(home)
            .service(search)
            .service(fs::Files::new("/static", "static").show_files_listing())
    })
    .bind((SERVER_DOMAIN, SERVER_PORT))?
    .run()
    .await
}

fn read_file(file_name: &str) -> Vec<String> {
    let answer_file = File::open(file_name).unwrap();
    let reader = BufReader::new(answer_file);

    reader
        .lines()
        .map(|l| l.expect("Could not parse line"))
        .collect()
}

fn get_answers() -> Vec<String> {
    read_file("allowed_answers.txt")
}

fn get_guesses() -> Vec<String> {
    let mut answers = get_answers();
    let mut guesses = read_file("allowed_guesses.txt");

    guesses.append(&mut answers);

    guesses
}

#[get("/")]
async fn home() -> impl Responder {
    info!("Home");

    with_header(String::from(
        "<p>Right click on the address bar to install the search engine.</p>",
    ))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
}

#[get("/search")]
async fn search(query: web::Query<SearchQuery>) -> impl Responder {
    let search = &query.q;
    info!("Searching: {search}");
    with_header(format!("Content: {}", search))
}

/// Returns provided content as html body together with an opensearch html header
fn with_header(content: String) -> impl Responder {
    HttpResponse::Ok().body(
    format!("
    <html>
        <head>
            <link rel=\"search\" type=\"application/opensearchdescription+xml\" title=\"searchGame\" href=\"http://{SERVER_DOMAIN}:{SERVER_PORT}/static/opensearch.xml\" />
        </head>
        <body>
            {content}
        </body>
    </html>
    "))
}

fn maybe_error(guesses: &[String], guess: String) -> Option<String> {
    let guess_length = guess.len();
    if guess_length < 5 {
        return Some(String::from("less than 5 characters"));
    } else if guess_length > 5 {
        return Some(String::from("greater than 5 characters"));
    } else if !guesses.contains(&guess) {
        return Some(String::from("not in wordlist"));
    }
    None
}

#[derive(Debug, Serialize)]
struct SearchSuggestionResponse(String, Vec<String>);

#[get("/game")]
async fn game(app_state: web::Data<AppState>, query: web::Query<SearchQuery>) -> impl Responder {
    info!("Game on!");
    let search_query = query.q.to_string();
    let mut guesses = query.q.split(&['.', ' ']).peekable();

    info!("Gaming: {search_query}");

    let mut response = Vec::new();

    if guesses.peek().is_none() {
        info!("No guess");
        response.push(String::from("Enter 5-letter guesses separated by spaces"));
    }

    for guess in guesses {
        match maybe_error(&app_state.guesses, guess.to_string()) {
            None => {
                let result = to_result(guess, &app_state.word);
                let mut s = format!("{guess} | {result}");

                if result == format!("{GREEN}{GREEN}{GREEN}{GREEN}{GREEN}") {
                    s = format!("{guess} | CORRECT! ✅");
                }

                info!("No error: {s}");

                response.push(s)
            }
            Some(error_msg) => response.push(format!("{guess} | ERROR: {error_msg}")),
        }
    }

    info!("Game response: {response:?}");

    HttpResponse::Ok()
        .content_type("application/x-suggestions+json")
        .json(SearchSuggestionResponse(search_query, response))
}

fn to_result(guess: &str, word: &str) -> String {
    let mut chars = vec![WHITE; 5];
    let mut counts = HashMap::<char, i32>::new();

    for c in word.chars() {
        counts.insert(c, 0);
    }

    for (idx, (g, a)) in zip(guess.chars(), word.chars()).enumerate() {
        if g == a {
            chars[idx] = GREEN
        } else if let Some(c) = counts.get_mut(&a) {
            *c += 1;
        }
    }

    for (idx, g) in guess.chars().enumerate() {
        if counts.contains_key(&g) && counts[&g] > 0 && chars[idx] == WHITE {
            chars[idx] = YELLOW;
            if let Some(c) = counts.get_mut(&g) {
                *c -= 1;
            }
        }
    }

    chars.join("")
}
