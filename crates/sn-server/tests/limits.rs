//! Gränser för anrop och ordningen kontroller sker i.
//!
//! Databaspoolen ansluter aldrig. Varje test visar alltså att anropet stoppas innan
//! appkoden når databasen, och att kroppen inte tolkas före inloggning och CSRF-kontroll.

use std::{
    convert::Infallible,
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    Router,
    body::{Body, Bytes},
    http::{Request, StatusCode, header},
};
use http_body::Frame;
use sn_server::{AppState, BODY_TIMEOUT, Config, MAX_BODY_BYTES, app};
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

fn test_app() -> Router {
    // Port 1 svarar inte. Poolen är lat, så den försöker bara ansluta om en hanterare frågar.
    let database_url = "postgres://ingen:ingen@127.0.0.1:1/ingen".to_owned();
    let db = PgPoolOptions::new().connect_lazy(&database_url).expect("giltig URL");
    let config = Config {
        database_url,
        bind: "127.0.0.1:0".parse().expect("giltig adress"),
        dev_login: true,
        cookie_secure: false,
        web_dist: None,
        session_hours: 12,
        public_url: "http://127.0.0.1:0".into(),
        oidc: None,
    };
    app(&AppState::new(db, config).expect("appstate"))
}

fn post(path: &str) -> axum::http::request::Builder {
    Request::post(path).header(header::CONTENT_TYPE, "application/json").header("x-sn-csrf", "1")
}

const VIEWS: &str = "/v1/bodies/00000000-0000-0000-0000-000000000000/views";

#[tokio::test]
async fn too_large_content_length_is_rejected_before_anything_else() {
    // Ingen sessionscookie och ingen CSRF-header: storleken avgör ändå först, utan att kroppen läses.
    let req = Request::post(VIEWS)
        .header(header::CONTENT_LENGTH, MAX_BODY_BYTES + 1)
        .body(Body::from(vec![b' '; MAX_BODY_BYTES + 1]))
        .expect("request");
    let res = test_app().oneshot(req).await.expect("svar");
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn too_large_body_without_length_is_stopped_while_reading() {
    // Utan Content-Length räknas bytes medan kroppen läses. Inloggning med personas tolkar JSON
    // före databasfrågan, så 413 visar att läsningen avbröts vid gränsen.
    let body = format!(r#"{{"account_id":"{}"}}"#, "0".repeat(MAX_BODY_BYTES));
    let req = post("/auth/dev/login").body(Body::from(body)).expect("request");
    let res = test_app().oneshot(req).await.expect("svar");
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn login_is_checked_before_the_body_is_parsed() {
    // Trasig JSON utan inloggning ger 401, inte 400: kroppen tolkas aldrig för en anonym användare.
    let req = post(VIEWS).body(Body::from("{ inte json")).expect("request");
    let res = test_app().oneshot(req).await.expect("svar");
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn csrf_is_checked_before_the_body_is_parsed() {
    let req = Request::post(VIEWS)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{ inte json"))
        .expect("request");
    let res = test_app().oneshot(req).await.expect("svar");
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn normal_sizes_pass_the_limit() {
    // En halv megabyte stoppas inte av storleksgränsen. Svaret kommer från inloggningskontrollen.
    let size = MAX_BODY_BYTES / 2;
    let req = post(VIEWS).header(header::CONTENT_LENGTH, size).body(Body::from(vec![b' '; size])).expect("request");
    let res = test_app().oneshot(req).await.expect("svar");
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// En kropp som aldrig blir klar, som när en klient skickar en byte i taget för att hålla kvar resurser.
struct Stalled;

impl http_body::Body for Stalled {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        Poll::Pending
    }
}

#[tokio::test(start_paused = true)]
async fn a_stalled_body_is_cut_off() {
    let started = tokio::time::Instant::now();
    let req = post("/auth/dev/login").body(Body::new(Stalled)).expect("request");
    let res = test_app().oneshot(req).await.expect("svar");
    assert!(res.status().is_client_error(), "fick {}", res.status());
    assert_eq!(started.elapsed().as_secs(), BODY_TIMEOUT.as_secs());
}
