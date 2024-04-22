mod links;
mod static_content;

use aws_config::BehaviorVersion;
use axum::{
    extract::{Path, Query, Request, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::get,
    Router,
};
use serde::Deserialize;

#[derive(Clone)]
struct AppState {
    links: links::LinkStorage,
}

#[tokio::main]
async fn main() {
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let client = aws_sdk_dynamodb::Client::new(&config);
    let link_storage = links::LinkStorage::new(
        client,
        std::env::var("TABLE").expect("Missing TABLE environment variable"),
    );

    let state = AppState {
        links: link_storage,
    };

    let app = Router::new()
        .route("/shorten-link", get(shorten_link))
        .route("/:key", get(perform_redirect))
        .route("/", get(index))
        .fallback(|_req: Request| async { Redirect::temporary("/") })
        .with_state(state);

    // In dev mode run like a regular axum app.
    #[cfg(debug_assertions)]
    {
        let listener = tokio::net::TcpListener::bind("localhost:3000")
            .await
            .unwrap();
        axum::serve(listener, app).await.unwrap();
    }

    // In release mode use the lambda adapter.
    #[cfg(not(debug_assertions))]
    {
        let app = tower::ServiceBuilder::new()
            .layer(axum_aws_lambda::LambdaLayer::default())
            .service(app);

        lambda_http::run(app).await.unwrap();
    }
}

async fn perform_redirect(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Redirect, AppResponse> {
    let link = state.links.get_link_for_key(key).await;
    match link {
        Ok(link) => Ok(Redirect::permanent(&link)),
        Err(links::Error::NotFound) => Err(AppResponse::NotFound),
        Err(links::Error::ServerError) => Err(AppResponse::InternalServerError),
    }
}

#[derive(Deserialize)]
struct CreateLinkQuery {
    link: String,
}

async fn shorten_link(
    State(state): State<AppState>,
    query: Query<CreateLinkQuery>,
) -> Result<String, AppResponse> {
    // Ensure the link starts with `http://` or `https://` so the redirect isn't interpreted as a relative link.
    let mut link: String = query.link.clone();
    if !link.starts_with("https://") && !link.starts_with("http://") {
        link.insert_str(0, "https://")
    }

    let key = state.links.create_short_link(link).await;
    match key {
        Ok(key) => {
            let short_link = format!("/{}", key);
            Ok(short_link)
        }
        Err(_) => Err(AppResponse::InternalServerError),
    }
}

async fn index() -> Html<&'static [u8]> {
    Html(static_content::INDEX_HTML)
}

enum AppResponse {
    InternalServerError,
    NotFound,
}

impl IntoResponse for AppResponse {
    fn into_response(self) -> axum::response::Response {
        let code = match self {
            AppResponse::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            AppResponse::NotFound => StatusCode::NOT_FOUND,
        };
        let body = match self {
            AppResponse::InternalServerError => static_content::ERROR_HTML,
            AppResponse::NotFound => static_content::NOT_FOUND_HTML,
        };

        (code, Html(body)).into_response()
    }
}
