use axum::body::StreamBody;
use axum::extract::Path;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{AppendHeaders, Html, IntoResponse};
use axum::routing::{any, get};
use axum::{Extension, Router};
use dioxus::prelude::*;
use rand::Rng;
use std::collections::HashMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use tower_http::trace::TraceLayer;

type FileTable = Arc<HashMap<String, (PathBuf, u64)>>;

pub async fn run_server(socket: TcpListener, files: &[PathBuf]) {
    let mut file_table = HashMap::with_capacity(files.len());
    for f in files {
        tracing::debug!("{}", f.display());
        file_table.insert(
            format!("{:x}", md5::compute(f.display().to_string())),
            (f.clone(), tokio::fs::metadata(f).await.unwrap().len()),
        );
    }

    for (k, v) in &file_table {
        tracing::debug!("Registered file : {k} -> {v:?}");
    }

    let app: _ = Router::new()
        .route("/", get(app_endpoint))
        .route("/:file_id", any(get_file))
        .layer(Extension(Arc::new(file_table)))
        .layer(TraceLayer::new_for_http());

    tracing::debug!("listening on {}", socket.local_addr().unwrap());
    axum::Server::from_tcp(socket)
        .unwrap()
        .serve(app.into_make_service())
        .await
        .unwrap();
}

const BUFFER_SIZE: usize = 8 * 1024 * 1024;

async fn get_file(
    Path(file_id): Path<String>,
    file_table: Extension<FileTable>,
) -> impl IntoResponse {
    if let Some((path, size)) = file_table.get(&file_id) {
        let file = File::open(path).await.unwrap();
        let body = StreamBody::new(ReaderStream::with_capacity(file, BUFFER_SIZE));
        let headers = AppendHeaders([
            (CONTENT_TYPE, "application/octet-stream".into()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename={:?}", path.file_name().unwrap()),
            ),
            (CONTENT_LENGTH, size.to_string()),
        ]);
        Ok((headers, body))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            format!("File not found: {}", file_id),
        ))
    }
}

async fn app_endpoint(file_table: Extension<FileTable>) -> Html<String> {
    Html(dioxus::ssr::render_lazy(rsx! {
        style { [include_str!("./style.css")] }
        script {
            src: "https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.2.0/js/all.min.js",
            integrity: "sha512-naukR7I+Nk6gp7p5TMA4ycgfxaZBJ7MO5iC3Fp6ySQyKFHOGfpkSZkYVWV5R7u7cfAicxanwYQ5D1e17EfJcMA==",
            crossorigin: "anonymous",
        }
        main {
            div { class: "items",
                div { class: "items-head",
                    h1 { "Files" }
                    hr {}
                }
                div { class: "items-body",
                    file_table.iter().map(|(k, (path, size))| {
                        let file_name = path.file_name().unwrap();
                        let delay = rand::thread_rng().gen_range(0..750);
                        let duration = rand::thread_rng().gen_range(500..1500);
                        let icon_size = ((*size as f32).log10().clamp(1.0, 10.0) / 5.0 + 0.8) * 20.0;
                        let size_display = human_bytes::human_bytes(*size as f64);
                        rsx! (
                            a { href: "/{k}",
                                div { class: "items-body-content", key: "{k}",
                                    span { "{file_name:?}" }
                                    span { style: "color: grey", "{size_display}" }
                                    i { class: "fas fa-file-download", style: "font-size: {icon_size}px; animation-delay: {delay}ms; animation-duration: {duration}ms;" }
                                }
                            }
                        )
                    })
                }
            }
        }
    }))
}
