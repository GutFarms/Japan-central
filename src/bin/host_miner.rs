//! Desktop demo of the same scrypt miner core (no ESP hardware required).
//!
//! Credentials are saved to `scrypt-miner-config.bin` and auto-loaded next run.
//!
//! ```text
//! cargo run --no-default-features --features host --bin host-miner --release
//! cargo run --no-default-features --features host --bin host-miner --release -- --clear
//! ```

use std::io::{self, Write as _};
use std::time::Instant;

use esp32_s3_scrypt_miner::config::{PoolConfig, SetupField};
use esp32_s3_scrypt_miner::miner::{hash_to_hex, ScryptMiner, SCRYPT_LOG_N, SCRYPT_N};
use esp32_s3_scrypt_miner::persist::{self, HOST_CONFIG_PATH};

fn main() {
    let mut difficulty = 4u8;
    let mut cfg = PoolConfig::new();
    let mut clear_saved = false;
    let mut skip_save = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" | "-a" => {
                if let Some(v) = args.next() {
                    cfg.set(SetupField::Address, &v).expect("address");
                }
            }
            "--password" | "-p" => {
                if let Some(v) = args.next() {
                    cfg.set(SetupField::Password, &v).expect("password");
                }
            }
            "--stratum" | "-s" => {
                if let Some(v) = args.next() {
                    cfg.set(SetupField::Stratum, &v).expect("stratum");
                }
            }
            "--difficulty" | "-d" => {
                if let Some(v) = args.next() {
                    difficulty = v.parse().unwrap_or(4);
                }
            }
            "--clear" | "--factory" => clear_saved = true,
            "--no-save" => skip_save = true,
            other if other.parse::<u8>().is_ok() => {
                difficulty = other.parse().unwrap();
            }
            other => {
                eprintln!("unknown arg: {other}");
            }
        }
    }

    println!("SCRYPT host miner");
    println!("  N={SCRYPT_N} (2^{SCRYPT_LOG_N})  r=1  p=1");
    println!("  demo difficulty: {difficulty} leading zero nibbles");
    println!("  config file: {HOST_CONFIG_PATH}");
    println!();

    if clear_saved {
        let _ = persist::clear();
        println!("Cleared saved credentials.");
    }

    let from_file = if cfg.is_complete() {
        false
    } else if let Ok(saved) = persist::load() {
        println!("Loaded saved credentials from {HOST_CONFIG_PATH}");
        cfg = saved;
        true
    } else {
        false
    };

    if !cfg.is_complete() {
        println!("Enter pool credentials (required before mining):");
        prompt_field(&mut cfg, SetupField::Address);
        prompt_field(&mut cfg, SetupField::Password);
        prompt_field(&mut cfg, SetupField::Stratum);
    }

    if !skip_save && !from_file {
        match persist::save(&cfg) {
            Ok(()) => println!("Saved credentials to {HOST_CONFIG_PATH}"),
            Err(e) => eprintln!("WARNING: could not save credentials: {e}"),
        }
    } else if from_file && cfg.is_complete() {
        // Refresh file if CLI overrode some fields after load — only when user passed flags.
        // Keep simple: always re-save complete config so edits stick.
        let _ = persist::save(&cfg);
    }

    println!();
    println!("Using{}:", if from_file { " (from save)" } else { "" });
    println!("  address  = {}", cfg.address);
    println!("  password = {}", cfg.password_masked());
    println!("  stratum  = {}", cfg.stratum);
    println!();

    let mut miner = ScryptMiner::new_demo(difficulty);
    let start = Instant::now();
    let mut last_report = Instant::now();
    let mut window_hashes = 0u64;

    loop {
        let result = miner.mine_one();
        window_hashes += 1;

        if result.is_share {
            let mut hex = heapless::String::<128>::new();
            hash_to_hex(&result.hash, 16, &mut hex);
            println!(
                "*** SHARE nonce={:08x} hash={hex} address={} stratum={} total_shares={}",
                result.nonce,
                cfg.address,
                cfg.stratum,
                miner.stats().shares
            );
        }

        if last_report.elapsed().as_millis() >= 1000 {
            let stats = miner.stats();
            let secs = start.elapsed().as_secs_f64().max(0.001);
            let hps = stats.hashes as f64 / secs;
            let mut best = heapless::String::<128>::new();
            hash_to_hex(&stats.best_hash, 8, &mut best);
            println!(
                "rate={hps:.2} H/s  nonce={:08x}  shares={}  best={best}  window={window_hashes}  addr={}  stratum={}",
                stats.nonce, stats.shares, cfg.address, cfg.stratum
            );
            last_report = Instant::now();
            window_hashes = 0;
        }
    }
}

fn prompt_field(cfg: &mut PoolConfig, field: SetupField) {
    if !cfg.get(field).is_empty() {
        return;
    }
    let stdin = io::stdin();
    loop {
        print!("{} ({}): ", field.label(), field.prompt());
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() {
            continue;
        }
        match cfg.set(field, line.trim()) {
            Ok(()) => break,
            Err(e) => eprintln!("  error: {e}"),
        }
    }
}
