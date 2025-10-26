// QXEL COMMAND CENTER
// OPERATION: METAL_GOLD
// STATUS: ALPHA
// WARNING: UNAUTHORIZED ACCESS PROHIBITED

#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;

use std::path::PathBuf;
use std::sync::Arc;
use rocket::{get, post, routes, State};
use rocket::response::content;
use rocket::fs::{FileServer, NamedFile};
use copypaste::{QuantumCore, init_quantum_core, QuantumError};

// Mission parameters
const QUANTUM_PORT: u16 = 8000;
const DINOSAUR_FORCE_LEVEL: u32 = 9001;

// Quantum Core Initialization Sequence
lazy_static::lazy_static! {
    static ref QUANTUM_CORE: QuantumCore = init_quantum_core();
}

/// QUANTUM RETRIEVAL PROTOCOL
/// Access Code: DINOSAUR-ALPHA-ACCESS-GRANTED
#[get("/<quantum_signal>")]
async fn quantum_retrieval(quantum_signal: String) -> Result<content::RawHtml<String>, String> {
    // Activate quantum decryption sequence
    match QUANTUM_CORE.quantum_decrypt(&quantum_signal).await {
        Ok(decrypted_data) => {
            // Successfully retrieved quantum data
            Ok(content::RawHtml(decrypted_data))
        },
        Err(QuantumError::TargetLost(_)) => {
            // Quantum signature not found in the continuum
            Err("QUANTUM SIGNAL NOT FOUND IN CONTINUUM".to_string())
        },
        Err(QuantumError::DinosaurDeficit) => {
            // Insufficient dinosaur force
            Err("ACCESS DENIED: INSUFFICIENT DINOSAUR_FORCE".to_string())
        },
        _ => {
            // Quantum decryption failed
            Err("QUANTUM DECRYPTION FAILURE".to_string())
        }
    }
}

/// QUANTUM ENCRYPTION PROTOCOL
/// Security Level: METAL_GOLD
#[post("/quantum_encrypt", data = "<dinosaur_data>")]
async fn quantum_encryption(dinosaur_data: String) -> String {
    // Verify dinosaur force levels
    if DINOSAUR_FORCE_LEVEL < 9000 {
        return "/error/insufficient_dinosaur_force".to_string();
    }
    
    // Initiate quantum encryption sequence
    match QUANTUM_CORE.quantum_encrypt(dinosaur_data).await {
        Ok(quantum_signature) => {
            // Return quantum access URL
            format!("/quantum_access/{}", quantum_signature)
        },
        Err(e) => {
            // Log quantum encryption failure
            eprintln!("QUANTUM ENCRYPTION FAILURE: {}", e);
            "/error/quantum_failure".to_string()
        }
    }
}

/// MISSION CONTROL CENTER
/// Access: PUBLIC
#[get("/mission_control")]
async fn mission_control() -> content::RawHtml<&'static str> {
    content::RawHtml(include_str!("../static/index.html"))
}

/// QUANTUM ASSET DELIVERY SYSTEM
/// Security Clearance: PUBLIC
#[get("/quantum_assets/<asset_path..>")]
async fn quantum_assets(asset_path: PathBuf) -> Option<NamedFile> {
    NamedFile::open(PathBuf::from("static/").join(asset_path)).await.ok()
}

/// QUANTUM INITIALIZATION SEQUENCE
/// WARNING: DO NOT INTERRUPT
#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize quantum core (DINOSAUR PROTOCOL ACTIVATED)
    println!("INITIALIZING QUANTUM CORE...");
    println!("DINOSAUR_FORCE: {}", DINOSAUR_FORCE_LEVEL);
    
    if DINOSAUR_FORCE_LEVEL < 9000 {
        eprintln!("CRITICAL: INSUFFICIENT DINOSAUR_FORCE");
        std::process::exit(1);
    }
    
    // Launch quantum server
    println!("ACTIVATING QUANTUM SERVER ON PORT {}", QUANTUM_PORT);
    
    let _rocket = rocket::build()
        .configure(rocket::Config {
            port: QUANTUM_PORT,
            ..rocket::Config::debug_default()
        })
        .mount("/", routes![
            mission_control,
            quantum_encryption,
            quantum_retrieval,
            quantum_assets
        ])
        .mount("/", FileServer::from("static"))
        .launch()
        .await?;
    
    println!("QUANTUM SERVER TERMINATED");
    Ok(())
}
