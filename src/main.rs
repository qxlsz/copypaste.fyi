use std::path::{Path, PathBuf};
use rocket::{get, post, routes, State};
use rocket::response::content;
use rocket::fs::{FileServer, NamedFile};
use rand::{distributions::Alphanumeric, Rng};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use chrono::NaiveDateTime;
use std::env;
use std::error::Error;

type DbPool = SqlitePool;

#[get("/<id>")]
async fn get_paste(pool: &State<DbPool>, id: String) -> Option<content::RawHtml<String>> {
    match sqlx::query_scalar::<_, String>("SELECT content FROM pastes WHERE id = ?")
        .bind(&id)
        .fetch_optional(&**pool)
        .await
    {
        Ok(Some(content)) => Some(content::RawHtml(content)),
        _ => None,
    }
}

#[post("/", data = "<content>")]
async fn create_paste(pool: &State<DbPool>, content: String) -> String {
    let id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();

    if let Err(e) = sqlx::query(
        "INSERT INTO pastes (id, content) VALUES (?, ?)"
    )
    .bind(&id)
    .bind(&content)
    .execute(&**pool)
    .await {
        eprintln!("Failed to insert paste: {}", e);
        return "/error".to_string();
    }

    format!("/{}", id)
}

#[get("/")]
async fn index() -> content::RawHtml<&'static str> {
    content::RawHtml(include_str!("../static/index.html"))
}

#[get("/static/<file..>")]
async fn static_files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).await.ok()
}

async fn init_db() -> Result<DbPool, Box<dyn Error>> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/pastes.db".to_string());
    
    // Create data directory if it doesn't exist
    if let Some(parent) = Path::new(&database_url).parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    
    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;
    
    Ok(pool)
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize database
    let db_pool = init_db().await?;
    
    let _rocket = rocket::build()
        .manage(db_pool)
        .mount("/", routes![index, create_paste, get_paste, static_files])
        .mount("/", FileServer::from("static"))
        .launch()
        .await?;
        
    Ok(())
}
