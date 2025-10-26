use axum::{
    routing::{get, post},
    Router, response::Html, extract::{Path as AxumPath, State}, 
    http::StatusCode, response::IntoResponse, body::Bytes
};
use std::{net::SocketAddr, path::Path, sync::Arc};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::{SqlitePool, SqlitePoolOptions}, migrate::MigrateDatabase, Sqlite};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
struct Paste {
    id: String,
    content: String,
    created_at: i64,
}

struct AppState {
    db: SqlitePool,
}

async fn get_paste(
    AxumPath(id): AxumPath<String>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let paste = sqlx::query_as::<_, (String,)>(
        "SELECT content FROM pastes WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some((content,)) = paste {
        Ok(Html(content))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn create_paste(
    State(state): State<Arc<AppState>>,
    body: Bytes,
) -> Result<String, StatusCode> {
    let content = String::from_utf8(body.to_vec()).map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    sqlx::query(
        "INSERT INTO pastes (id, content, created_at) VALUES (?, ?, ?)"
    )
    .bind(&id)
    .bind(&content)
    .bind(created_at)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(format!("/{id}"))
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn serve_static(AxumPath(path): AxumPath<String>) -> impl IntoResponse {
    match fs::read_to_string(format!("static/{}", path)).await {
        Ok(content) => {
            let content_type = if path.ends_with(".js") {
                "application/javascript"
            } else if path.ends_with(".css") {
                "text/css"
            } else {
                "text/plain"
            };
            
            ([(axum::http::header::CONTENT_TYPE, content_type)], content).into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create data directory if it doesn't exist
    let data_dir = Path::new("data");
    if !data_dir.exists() {
        fs::create_dir_all(data_dir).await?;
    }
    
    // Initialize SQLite database
    let db_url = "sqlite:data/pastes.db";
    if !Sqlite::database_exists(db_url).await? {
        Sqlite::create_database(db_url).await?;
    }
    
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(db_url)
        .await?;
    
    // Initialize database
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS pastes (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )"#
    )
    .execute(&pool)
    .await?;
    
    let state = Arc::new(AppState { db: pool });
    
    // Set up routes
    let app = Router::new()
        .route("/", get(index).post(create_paste))
        .route("/:id", get(get_paste))
        .route("/static/*path", get(serve_static))
        .with_state(state);
    
    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
    println!("Server running on http://{0}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}
