//! Desktop demo of the same scrypt miner core (no ESP hardware required).
//!
//! ```text
//! cargo run --no-default-features --features host --bin host-miner --release
//! ```

use std::time::Instant;

use esp32_s3_scrypt_miner::miner::{hash_to_hex, ScryptMiner, SCRYPT_LOG_N, SCRYPT_N};

fn main() {
    let difficulty = std::env::args()
        .nth(1)
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(4);

    println!("SCRYPT host miner");
    println!("  N={SCRYPT_N} (2^{SCRYPT_LOG_N})  r=1  p=1");
    println!("  demo difficulty: {difficulty} leading zero nibbles");
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
                "*** SHARE nonce={:08x} hash={hex} total_shares={}",
                result.nonce,
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
                "rate={hps:.2} H/s  nonce={:08x}  shares={}  best={best}  window={window_hashes}",
                stats.nonce, stats.shares
            );
            last_report = Instant::now();
            window_hashes = 0;
        }
    }
}
