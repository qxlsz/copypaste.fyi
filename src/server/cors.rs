use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{Header, Method, Status};
use rocket::{options, Request, Response};

const ALLOWED_METHODS: &str = "GET,POST,OPTIONS";
const ALLOWED_HEADERS: &str = "Content-Type,Authorization";
const EXPOSED_HEADERS: &str = "Content-Type";
const MAX_AGE_SECONDS: &str = "86400";

#[derive(Clone, Copy)]
pub struct Cors;

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "CORS headers",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        response.set_header(Header::new("Access-Control-Allow-Methods", ALLOWED_METHODS));
        response.set_header(Header::new("Access-Control-Allow-Headers", ALLOWED_HEADERS));
        response.set_header(Header::new(
            "Access-Control-Expose-Headers",
            EXPOSED_HEADERS,
        ));
        response.set_header(Header::new("Access-Control-Max-Age", MAX_AGE_SECONDS));

        if request.method() == Method::Options {
            response.set_status(Status::NoContent);
            response.set_header(Header::new("Content-Length", "0"));
        }
    }
}

#[options("/api/<_..>")]
pub fn api_preflight() -> Status {
    Status::NoContent
}
