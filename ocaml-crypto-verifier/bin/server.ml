open Cohttp
open Cohttp_lwt_unix
open Crypto_verifier

let port = try int_of_string (Sys.getenv "PORT") with _ -> 8001

(* Default to "::" (IPv6 any, dual-stack — also accepts IPv4). Fly.io's
   private 6PN network is IPv6-only, so an IPv4-only 0.0.0.0 bind is
   unreachable from other machines in the org. *)
let host = try Sys.getenv "HOST" with _ -> "::"

let json_content_type = ("Content-Type", "application/json")
(* A 1 MiB paste appears twice in an encryption verification request: once as
   JSON-escaped plaintext (up to 6x expansion for control bytes) and once as
   base64 ciphertext (~4/3x). Keep the verifier bounded while accepting the
   backend's full advertised limit plus envelope/key overhead. *)
let max_body_bytes = 8 * 1024 * 1024

let read_body_bounded (body : Cohttp_lwt.Body.t) =
  let stream = Cohttp_lwt.Body.to_stream body in
  let buffer = Buffer.create (min max_body_bytes 65_536) in
  let rec read total =
    Lwt.bind (Lwt_stream.get stream) (function
      | None -> Lwt.return (Ok (Buffer.contents buffer))
      | Some chunk ->
          let next_total = total + String.length chunk in
          if next_total > max_body_bytes then
            Lwt.return (Error `Too_large)
          else begin
            Buffer.add_string buffer chunk;
            read next_total
          end)
  in
  read 0

let respond_json status json =
  let body = Yojson.Safe.to_string json in
  Server.respond_string ~status ~headers:(Cohttp.Header.of_list [json_content_type]) ~body ()

let result_to_json (result : verification_result) =
  `Assoc [
    ("valid", `Bool result.valid);
    ("details", `String result.details);
    ("timestamp", `Float result.timestamp)
  ]

let handle_health (_req : Request.t) (_body : Cohttp_lwt.Body.t) =
  let result = health_check () in
  let json = `Assoc [
    ("status", `String "healthy");
    ("verifier", result_to_json result)
  ] in
  respond_json `OK json

let handle_verify_encryption (_req : Request.t) (body : Cohttp_lwt.Body.t) =
  Lwt.bind (read_body_bounded body) (function
    | Error `Too_large ->
      respond_json `Bad_request
        (`Assoc [("valid", `Bool false); ("details", `String "request body too large")])
    | Ok body_str ->
      let result = match encryption_verification_of_string body_str with
        | Ok ev -> verify_encryption ev
        | Error msg ->
            { valid = false; details = "Parse error: " ^ msg; timestamp = Unix.gettimeofday () }
      in
      respond_json `OK (result_to_json result))

let handle_verify_signature (_req : Request.t) (body : Cohttp_lwt.Body.t) =
  Lwt.bind (read_body_bounded body) (function
    | Error `Too_large ->
      respond_json `Bad_request
        (`Assoc [("valid", `Bool false); ("details", `String "request body too large")])
    | Ok body_str ->
      let result = match signature_verification_of_string body_str with
        | Ok sv -> verify_signature sv
        | Error msg ->
            { valid = false; details = "Parse error: " ^ msg; timestamp = Unix.gettimeofday () }
      in
      respond_json `OK (result_to_json result))

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
  (* `TCP (`Port _) alone binds IPv4-any; build a conduit context from
     [host] so the bind address is honoured. *)
  Lwt.bind (Conduit_lwt_unix.init ~src:host ()) (fun conduit_ctx ->
      let ctx = Cohttp_lwt_unix.Net.init ~ctx:conduit_ctx () in
      Server.create ~ctx ~mode:(`TCP (`Port port)) (Server.make ~callback ()))

let () =
  Lwt_main.run (start_server ())
