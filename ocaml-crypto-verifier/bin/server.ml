open Cohttp
open Cohttp_lwt_unix
open Crypto_verifier

let port = try int_of_string (Sys.getenv "PORT") with _ -> 8001
let host = try Sys.getenv "HOST" with _ -> "0.0.0.0"

let json_content_type = ("Content-Type", "application/json")

let respond_json status json =
  let body = Yojson.Safe.to_string json in
  Server.respond_string ~status ~headers:(Cohttp.Header.of_list [json_content_type]) ~body ()

let handle_health (_req : Request.t) (_body : Cohttp_lwt.Body.t) =
  let result = health_check () in
  let json = `Assoc [
    ("status", `String "healthy");
    ("verifier", `Assoc [
      ("valid", `Bool result.valid);
      ("details", `String result.details);
      ("timestamp", `Float result.timestamp)
    ])
  ] in
  respond_json `OK json

let handle_verify_encryption (_req : Request.t) (_body : Cohttp_lwt.Body.t) =
  let result = { valid = true; details = "Encryption verification placeholder"; timestamp = Unix.gettimeofday () } in
  let json = `Assoc [
    ("valid", `Bool result.valid);
    ("details", `String result.details);
    ("timestamp", `Float result.timestamp)
  ] in
  respond_json `OK json

let handle_verify_signature (_req : Request.t) (_body : Cohttp_lwt.Body.t) =
  let result = { valid = true; details = "Signature verification placeholder"; timestamp = Unix.gettimeofday () } in
  let json = `Assoc [
    ("valid", `Bool result.valid);
    ("details", `String result.details);
    ("timestamp", `Float result.timestamp)
  ] in
  respond_json `OK json

let callback _conn req body =
  let uri = req |> Request.uri |> Uri.path in
  let meth = req |> Request.meth in
  match meth, uri with
  | `GET, "/health" -> handle_health req body
  | `POST, "/verify/encryption" ->
      handle_verify_encryption req body
  | `POST, "/verify/signature" ->
      handle_verify_signature req body
  | _ ->
      let json = `Assoc [
        ("error", `String "Not found");
        ("path", `String uri);
        ("method", `String (Code.string_of_method meth));
        ("timestamp", `Float (Unix.gettimeofday ()))
      ] in
      respond_json `Not_found json

let start_server () =
  Logs.set_reporter (Logs_fmt.reporter ());
  Logs.set_level (Some Logs.Info);
  Logs.info (fun m -> m "Starting crypto verification server on %s:%d" host port);
  Server.create ~mode:(`TCP (`Port port)) (Server.make ~callback ())

let () =
  Lwt_main.run (start_server ())
