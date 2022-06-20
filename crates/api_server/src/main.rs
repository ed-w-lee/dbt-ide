use std::{net::SocketAddr, time::Duration};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        WebSocketUpgrade,
    },
    handler::Handler,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, get_service},
    Router, TypedHeader,
};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "dbt_ide_api_server=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .fallback(
            get_service(
                ServeDir::new("crates/api_server/assets").append_index_html_on_directories(true),
            )
            .handle_error(|error| async move {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Unhandled internal error: {}", error),
                )
            }),
        )
        // .route("/", get(handler))
        .route("/ws", get(ws_handler))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handler() -> Html<&'static str> {
    Html("Hello!")
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
) -> impl IntoResponse {
    if let Some(TypedHeader(user_agent)) = user_agent {
        tracing::debug!(user_agent = user_agent.as_str());
    }

    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    if let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(t) => {
                    tracing::debug!(message = "client sent text", body = t.as_str());
                }
                Message::Binary(_) => {
                    tracing::debug!(message = "client sent binary data");
                }
                Message::Ping(_) => {
                    tracing::debug!(message = "socket ping; automatically handled");
                }
                Message::Pong(_) => {
                    tracing::debug!(message = "socket pong; automatically handled");
                }
                Message::Close(_) => {
                    tracing::debug!(message = "client disconnected");
                    return;
                }
            }
        } else {
            tracing::debug!("failed to parse message; client disconnected");
            return;
        }
    }

    loop {
        if socket
            .send(Message::Text(String::from("hello there")))
            .await
            .is_err()
        {
            tracing::debug!("failed to send message; client disconnected");
            return;
        }

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "bad route :(")
}
