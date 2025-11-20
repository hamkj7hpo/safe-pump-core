// safe_pump/build.rs
// Generates MOTHERSHIP_PROGRAM_ID, SEED_COIN_ID, and INTERFACE_PROGRAM_ID at compile time
// Used by: safe_pump (mothership), safe_pump_interface, and any CPI clients

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=Anchor.toml");

    let anchor_toml = std::fs::read_to_string("Anchor.toml").expect("Failed to read Anchor.toml");

    let mut mothership_id = None;
    let mut seed_coin_id = None;
    let mut interface_id = None;

    for line in anchor_toml.lines() {
        if line.contains("safe_pump = \"") {
            mothership_id = line.split('"').nth(1).map(str::to_string);
        }
        if line.contains("seed_coin = \"") {
            seed_coin_id = line.split('"').nth(1).map(str::to_string);
        }
        if line.contains("safe_pump_interface = \"") {
            interface_id = line.split('"').nth(1).map(str::to_string);
        }
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_program_ids.rs");
    let mut f = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&dest_path)
        .expect("Failed to create generated_program_ids.rs");

    // These will fail to compile if missing — perfect safety
    writeln!(
        f,
        "pub const MOTHERSHIP_PROGRAM_ID: &str = \"{}\";",
        mothership_id.expect("safe_pump program ID not found in Anchor.toml")
    )
    .unwrap();

    writeln!(
        f,
        "pub const SEED_COIN_ID: &str = \"{}\";",
        seed_coin_id.expect("seed_coin program ID not found in Anchor.toml")
    )
    .unwrap();

    writeln!(
        f,
        "pub const INTERFACE_PROGRAM_ID: &str = \"{}\";",
        interface_id.expect("safe_pump_interface program ID not found in Anchor.toml")
    )
    .unwrap();

    // Re-export as Pubkey constants for convenience
    writeln!(f, "pub const MOTHERSHIP_PUBKEY: solana_program::pubkey::Pubkey = solana_program::pubkey!({});", 
        mothership_id.expect("duplicate")).unwrap();
    writeln!(f, "pub const SEED_COIN_PUBKEY: solana_program::pubkey::Pubkey = solana_program::pubkey!({});", 
        seed_coin_id.expect("duplicate")).unwrap();
    writeln!(f, "pub const INTERFACE_PUBKEY: solana_program::pubkey::Pubkey = solana_program::pubkey!({});", 
        interface_id.expect("duplicate")).unwrap();
}
