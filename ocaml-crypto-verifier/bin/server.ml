open Lwt
open Cohttp
open Cohttp_lwt_unix
open Crypto_verifier

let port = try int_of_string (Sys.getenv "PORT") with _ -> 8001
let host = try Sys.getenv "HOST" with _ -> "0.0.0.0"

let json_content_type = ("Content-Type", "application/json")

let respond_json status json =
  let body = Yojson.Safe.to_string json in
  Server.respond_string ~status ~headers:(Cohttp.Header.of_list [json_content_type]) ~body ()

let respond_error status message =
  let json = `Assoc [
    ("error", `String message);
    ("timestamp", `Float (Unix.gettimeofday ()))
  ] in
  respond_json status json

let handle_health _req _body =
  let result = health_check () in
  let json = `Assoc [
    ("status", `String "healthy");
    ("verifier", verification_result_to_yojson result)
  ] in
  respond_json `OK json

let handle_verify_encryption req body =
  try
    let json = Yojson.Safe.from_string body in
    let ev = encryption_verification_of_yojson json in
    match ev with
    | Ok ev ->
        let result = verify_encryption ev in
        let json = verification_result_to_yojson result in
        respond_json `OK json
    | Error msg -> respond_error `Bad_request ("Invalid request: " ^ msg)
  with
  | Yojson.Json_error msg -> respond_error `Bad_request ("JSON parse error: " ^ msg)
  | exn -> respond_error `Internal_server_error ("Server error: " ^ Printexc.to_string exn)

let handle_verify_signature req body =
  try
    let json = Yojson.Safe.from_string body in
    let sv = signature_verification_of_yojson json in
    match sv with
    | Ok sv ->
        let result = verify_signature sv in
        let json = verification_result_to_yojson result in
        respond_json `OK json
    | Error msg -> respond_error `Bad_request ("Invalid request: " ^ msg)
  with
  | Yojson.Json_error msg -> respond_error `Bad_request ("JSON parse error: " ^ msg)
  | exn -> respond_error `Internal_server_error ("Server error: " ^ Printexc.to_string exn)

let callback _conn req body =
  let uri = req |> Request.uri |> Uri.path in
  let meth = req |> Request.meth in
  match meth, uri with
  | `GET, "/health" -> handle_health req body
  | `POST, "/verify/encryption" ->
      Cohttp_lwt.Body.to_string body >>= handle_verify_encryption req
  | `POST, "/verify/signature" ->
      Cohttp_lwt.Body.to_string body >>= handle_verify_signature req
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
