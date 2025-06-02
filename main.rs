use axum::{
    extract::{Query, Multipart, DefaultBodyLimit},
    http::StatusCode,
    response::Json,
    routing::{get, post, get_service},
    Router,
};
use serde::Deserialize;
use std::{fs, net::SocketAddr, path::PathBuf, path::Path};
use tower_http::services::{ServeDir, ServeFile};

async fn find_available_port(start_port: u16) -> Option<u16> {
    (start_port..start_port + 100)
        .find(|port| {
            std::net::TcpListener::bind(("127.0.0.1", *port)).is_ok()
        })
}

#[tokio::main]
async fn main() {
    // Create files directory if it doesn't exist
    std::fs::create_dir_all("files").unwrap();

    let app = Router::new()
        .route("/", get_service(ServeFile::new("index.html")))
        .route("/files", get(list_files))
        .route("/upload", post(upload_file))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB limit
        .nest_service("/download", ServeDir::new("./files"));

    let port = find_available_port(3001)
        .await  // Add .await here
        .expect("No available ports found between 3001 and 3100");
    
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    println!("Listening on http://{}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Deserialize)]
struct FileQuery {
    r#type: Option<String>,
    search: Option<String>,
}

async fn list_files(Query(params): Query<FileQuery>) -> Result<Json<Vec<String>>, StatusCode> {
    let dir_path = PathBuf::from("./files");
    let entries = fs::read_dir(dir_path)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                match params.r#type.as_deref() {
                    Some("pdf") => ext.eq_ignore_ascii_case("pdf"),
                    Some("docx") => ext.eq_ignore_ascii_case("docx"),
                    Some("mp3") => ext.eq_ignore_ascii_case("mp3"),
                    Some("all") | None => true,
                    _ => false,
                }
            } else {
                params.r#type.is_none() || params.r#type.as_deref() == Some("all")
            }
        })
        .filter(|entry| {
            if let Some(search) = &params.search {
                entry
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&search.to_lowercase())
            } else {
                true
            }
        })
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    if entries.is_empty() {
        Ok(Json(vec![]))
    } else {
        Ok(Json(entries))
    }
}

async fn upload_file(mut multipart: Multipart) -> Result<StatusCode, String> {
    if let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name()
            .ok_or("No filename provided")?
            .to_string();
        let data = field.bytes().await
            .map_err(|e| e.to_string())?;
        
        let path = Path::new("files").join(&file_name);
        std::fs::write(path, data)
            .map_err(|e| e.to_string())?;
        
        Ok(StatusCode::CREATED)
    } else {
        Err("No file provided".to_string())
    }
}
