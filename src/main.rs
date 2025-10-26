use copypaste::{create_paste_store, PasteError, SharedPasteStore};
use rocket::fs::{FileServer, NamedFile};
use rocket::response::content;
use rocket::{get, post, routes, Build, Rocket, State};
use std::path::PathBuf;

#[get("/")]
async fn index() -> content::RawHtml<&'static str> {
    content::RawHtml(include_str!("../static/index.html"))
}

#[post("/", data = "<content>")]
async fn create(store: &State<SharedPasteStore>, content: String) -> Result<String, String> {
    store
        .create_paste(content)
        .await
        .map(|id| format!("/{}", id))
        .map_err(|_| "failed to create paste".to_string())
}

#[get("/<id>")]
async fn show(store: &State<SharedPasteStore>, id: String) -> Result<content::RawHtml<String>, String> {
    store
        .get_paste(&id)
        .await
        .map(content::RawHtml)
        .map_err(|err| match err {
            PasteError::NotFound(_) => "paste not found".to_string(),
        })
}

#[get("/static/<path..>")]
async fn static_files(path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(PathBuf::from("static").join(path)).await.ok()
}

fn build_rocket(store: SharedPasteStore) -> Rocket<Build> {
    rocket::build()
        .manage(store)
        .mount("/", routes![index, create, show, static_files])
        .mount("/", FileServer::from("static"))
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = rocket::Config {
        address: "0.0.0.0".parse()?,
        port: 8000,
        ..rocket::Config::debug_default()
    };

    build_rocket(create_paste_store())
        .configure(config)
        .launch()
        .await?;

    Ok(())
}
