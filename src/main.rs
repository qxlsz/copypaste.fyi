use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rocket::{get, post, routes};
use rocket::response::content;
use rocket::fs::{FileServer, NamedFile};
use rand::{distributions::Alphanumeric, Rng};
use std::path::{Path, PathBuf};
use lazy_static::lazy_static;

lazy_static! {
    static ref STORE: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
}

#[get("/<id>")]
async fn get_paste(id: String) -> Option<content::RawHtml<String>> {
    let store = STORE.lock().unwrap();
    store.get(&id).map(|content| content::RawHtml(content.clone()))
}

#[post("/", data = "<content>")]
async fn create_paste(content: String) -> String {
    let id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    
    // The created_at is not currently used, but we'll keep it for future use
    let _created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    
    let mut store = STORE.lock().unwrap();
    store.insert(id.clone(), content);
    
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

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    // Create data directory if it doesn't exist
    std::fs::create_dir_all("data").expect("Failed to create data directory");
    
    let _rocket = rocket::build()
        .mount("/", routes![index, create_paste, get_paste, static_files])
        .mount("/", FileServer::from("static"))
        .launch()
        .await?;
        
    Ok(())
}
