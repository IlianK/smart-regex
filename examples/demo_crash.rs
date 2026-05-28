//! Find stack overflow limits for recursive and loop parsers
//!
//! Build and run:
//!   cargo build --example demo_crash_worker
//!   cargo run --example demo_crash
//!   cargo run --example demo_crash --release

use std::io::Write;
use std::process::Command;
use std::env;
use std::time::Instant;

fn get_worker_path() -> std::path::PathBuf {
    let exe = env::current_exe().unwrap();
    let exe_dir = exe.parent().unwrap();
    let worker_name = if cfg!(windows) { "demo_crash_worker.exe" } else { "demo_crash_worker" };
    exe_dir.join(worker_name)
}

fn test_single(value: usize, use_loop: bool) -> bool {
    let worker_path = get_worker_path();
    
    if !worker_path.exists() {
        eprintln!("Worker not found at: {:?}", worker_path);
        return false;
    }
    
    let status = Command::new(&worker_path)
        .arg(value.to_string())
        .arg(use_loop.to_string())
        .status()
        .unwrap();
    
    status.success()
}

/// Binary search to find crash threshold
fn find_threshold(
    name: &str,
    use_loop: bool,
    min: usize,
    max: usize,
) -> usize {
    println!("\n=== Finding threshold for {} ===", name);
    println!("Searching between {} and {}...", min, max);
    
    let mut low = min;
    let mut high = max;
    let mut last_successful = min;
    let mut iterations = 0;
    const MAX_ITERATIONS: usize = 30;
    
    while low <= high && iterations < MAX_ITERATIONS {
        let mid = low + (high - low) / 2;
        iterations += 1;
        
        print!("  Testing {}: ", mid);
        std::io::stdout().flush().unwrap();
        
        let start = Instant::now();
        let success = test_single(mid, use_loop);
        let duration = start.elapsed().as_millis();
        
        if success {
            println!("✓ OK ({:.2}ms)", duration);
            last_successful = mid;
            low = mid + 1;
        } else {
            println!("✗ CRASH");
            high = mid - 1;
        }
    }
    
    println!("\n  Result: {} survives up to {} chars", name, last_successful);
    last_successful
}

fn main() {
    let mode = if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" };
    
    println!("\n");
    println!("================================================================================");
    println!("STACK OVERFLOW LIMITS: Recursive vs Loop Parser ({})", mode);
    println!("================================================================================");
    println!();
    
    let (rec_limit, loop_limit);
    
    if mode == "DEBUG" {
        // Debug mode ranges 
        rec_limit = find_threshold("Recursive a*", false, 2000, 10000);
        loop_limit = find_threshold("Loop a*", true, 2000, 15000);
    } else {
        // Release mode - higher ranges
        rec_limit = find_threshold("Recursive a*", false, 10000, 100000);
        loop_limit = find_threshold("Loop a*", true, 10000, 200000);
    }
    
    println!();
    println!("================================================================================");
    println!("SUMMARY - {} MODE", mode);
    println!("================================================================================");
    println!();
    println!("  {:<25} | {:>12} | {:>12}", "Test", "Recursive", "Loop");
    println!("  {:<25} | {:>12} | {:>12}", "-----------------------", "------------", "------------");
    println!("  {:<25} | {:>11} chars | {:>11} chars", "a* (shallow)", rec_limit, loop_limit);
    println!();
    
    if loop_limit > rec_limit {
        let ratio = loop_limit as f64 / rec_limit as f64;
        println!("  Loop survives {:.2}x longer for a*", ratio);
    }
    println!("================================================================================");
}