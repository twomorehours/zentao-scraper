use axum::{
    extract::Query,
    routing::{get, post},
    Form, Json, Router,
};
use reqwest::{header, ClientBuilder, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
};
use uuid::Uuid;
use zentao_scraper::SnBugs;

#[derive(Debug, Deserialize, Serialize)]
struct AllSnBug {
    sn_bugs: Vec<SnBugs>,
    active: i32,
    total: i32,
    ms_active: i32,
    ms_total: i32,
}

impl AllSnBug {
    fn new(sn_bugs: Vec<SnBugs>) -> Self {
        let mut active = 0;
        let mut total = 0;
        let mut ms_active = 0;
        let mut ms_total = 0;

        for sn_bug in sn_bugs.iter() {
            active += sn_bug.active();
            total += sn_bug.total();
            ms_active += sn_bug.ms_active();
            ms_total += sn_bug.ms_total();
        }
        AllSnBug {
            sn_bugs,
            active,
            total,
            ms_active,
            ms_total,
        }
    }
}

#[derive(Deserialize)]
struct SignIn {
    username: String,
    password: String,
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/bugs", get(list_bugs))
        .route("/login", post(login));

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn list_bugs(
    Query(params): Query<HashMap<String, String>>,
) -> (StatusCode, Json<Option<AllSnBug>>) {
    let verbose = params.get("verbose").map(|p| p.as_str()).unwrap_or("1");
    let filter = params.get("filter").map(|p| p.as_str()).unwrap_or("");
    let filters: HashSet<i32> = filter
        .split(',')
        .filter(|f| !f.trim().is_empty())
        .map(|f| f.trim().parse().unwrap())
        .collect();
    let keyword = params.get("keyword").map(Clone::clone);

    let mut headers = header::HeaderMap::new();
    headers.insert("User-Agent", header::HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36"));
    headers.insert(
        "Cookie",
        header::HeaderValue::from_str(&format!(
            "zentaosid={}",
            Uuid::new_v4().to_string().replace('-', "")
        ))
        .unwrap(),
    );

    let client = Arc::new(
        ClientBuilder::new()
            .default_headers(headers)
            .build()
            .unwrap(),
    );

    if let Err(e) = zentao_scraper::login("yuhao", "1qaz@WSX", &client).await {
        eprintln!("login error: {:?}", e);
        return (StatusCode::FORBIDDEN, Json(None));
    }

    let mut handles = vec![];
    for i in (1..=10).filter(|i| filters.is_empty() || filters.contains(i)) {
        let client = client.clone();
        let verbose = verbose.to_string();
        let keyword = keyword.clone();
        let h = tokio::spawn(async move {
            zentao_scraper::get_sx(i, &client, verbose == "1", keyword).await
        });
        handles.push(h);
    }
    let mut bugs_vec = vec![];
    for h in handles {
        match h.await {
            Ok(Ok(bugs)) => {
                if bugs.total() > 0 {
                    bugs_vec.push(bugs)
                }
            }
            Ok(Err(e)) => {
                eprintln!("Get Zentao Error: {:?}", e);
                continue;
            }
            Err(e) => {
                eprintln!("Join Error: {:?}", e);
                continue;
            }
        }
    }

    (StatusCode::OK, Json(Some(AllSnBug::new(bugs_vec))))
}

async fn login(Form(sign_in): Form<SignIn>) -> (StatusCode, Json<Option<Value>>) {
    let zentaosid = Uuid::new_v4().to_string().replace('-', "");
    let mut headers = header::HeaderMap::new();
    headers.insert("User-Agent", header::HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36"));
    headers.insert(
        "Cookie",
        header::HeaderValue::from_str(&format!("zentaosid={zentaosid}",)).unwrap(),
    );
    let client = ClientBuilder::new()
        .default_headers(headers)
        .build()
        .unwrap();
    if let Err(e) = zentao_scraper::login(&sign_in.username, &sign_in.password, &client).await {
        eprintln!("login error: {:?}", e);
        return (StatusCode::FORBIDDEN, Json(None));
    }
    (
        StatusCode::OK,
        Json(Some(serde_json::json!({ "token": zentaosid }))),
    )
}
