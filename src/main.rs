use std::path::PathBuf;
use std::sync::Arc;

use copypaste::{create_paste_store, PasteError, SharedPasteStore};
use rocket::fs::{FileServer, NamedFile};
use rocket::response::content;
use rocket::{get, post, routes, State};

#[get("/<id>")]
async fn get_paste(store: &State<SharedPasteStore>, id: String) -> Result<content::RawHtml<String>, String> {
    store
        .get_paste(&id)
        .await
        .map(content::RawHtml)
        .map_err(|err| match err {
            PasteError::NotFound(_) => "Paste not found".to_string(),
        })
}

#[post("/", data = "<content>")]
async fn create_paste(store: &State<SharedPasteStore>, content: String) -> Result<String, String> {
    store
        .create_paste(content)
        .await
        .map(|id| format!("/{}", id))
        .map_err(|_| "Failed to save paste".to_string())
}

#[get("/")]
async fn index() -> content::RawHtml<&'static str> {
    content::RawHtml(include_str!("../static/index.html"))
}

#[get("/static/<path..>")]
async fn static_files(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(PathBuf::from("static").join(path)).await.ok()
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store: SharedPasteStore = create_paste_store();

    let config = rocket::Config {
        address: "0.0.0.0".parse()?,
        port: 8000,
        ..rocket::Config::debug_default()
    };

    rocket::build()
        .configure(config)
        .manage(store)
        .mount("/", routes![index, create_paste, get_paste, static_files])
        .mount("/", FileServer::from("static"))
        .launch()
        .await?;

    Ok(())
}
