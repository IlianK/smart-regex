//! Find stack overflow limits for recursive and loop 
//!
//! Run: 
//!   cargo run --bin crash_demo           # Debug 
//!   cargo run --bin crash_demo --release # Release 

use std::io::Write;
use std::process::Command;
use std::env;
use std::time::Instant;

fn test_single(value: usize, is_depth: bool, use_loop: bool) -> (bool, u128) {
    let exe = env::current_exe().unwrap();
    let exe_dir = exe.parent().unwrap();
    
    let worker_name = if cfg!(windows) { "crash_demo_worker.exe" } else { "crash_demo_worker" };
    let worker_path = exe_dir.join(worker_name);
    
    if !worker_path.exists() {
        eprintln!("Worker not found at: {:?}", worker_path);
        return (false, 0);
    }
    
    let start = Instant::now();
    let status = Command::new(&worker_path)
        .arg(value.to_string())
        .arg(is_depth.to_string())
        .arg(use_loop.to_string())
        .status()
        .unwrap();
    let duration = start.elapsed().as_millis();
    
    (status.success(), duration)
}

fn test_parser(name: &str, use_loop: bool, test_name: &str, values: &[usize], is_depth: bool) -> usize {
    println!("\n=== {} - {} ===", name, test_name);
    
    let mut last_successful = 0;
    let mut last_time = 0;
    
    for &val in values {
        print!("  {}: ", val);
        std::io::stdout().flush().unwrap();
        
        let (success, duration) = test_single(val, is_depth, use_loop);
        
        if success {
            println!("OK ({:.2}ms)", duration);
            last_successful = val;
            last_time = duration;
        } else {
            println!("STACK OVERFLOW");
            if last_successful > 0 {
                println!("\n  Limit: {} ({}ms at last success)", last_successful, last_time);
            } else {
                println!("  Limit: < {}", val);
            }
            return last_successful;
        }
    }
    
    println!("\n  Survived all tests up to {} ({}ms)", values.last().unwrap(), last_time);
    *values.last().unwrap()
}

fn main() {
    let mode = if cfg!(debug_assertions) { "DEBUG" } else { "RELEASE" };
    
    println!("\n");
    println!("================================================================================");
    println!("STACK OVERFLOW LIMITS: Recursive vs Loop Parser ({})", mode);
    println!("================================================================================");
    println!();
    
    let (rec_a, loop_a, rec_deep, loop_deep);
    
    if mode == "DEBUG" {
        // =============================================================
        // DEBUG MODE 
        // =============================================================
        
        // Recursive a*
        let rec_a_values = [1000, 1100, 1200, 1250, 1300];
        
        // Loop a*
        let loop_a_values = [1000, 1100, 1200, 1250, 1300, 2000, 2100, 2150, 2200];
        
        // Recursive deep
        let rec_deep_values = [1000, 1100, 1200, 1250, 1300];
        
        // Loop deep
        let loop_deep_values = [1000, 1100, 1200, 1250, 1300, 2000, 2200, 2400, 2500];
        
        println!("--- Shallow Expression (a*) ---");
        let r_a = test_parser("Recursive", false, "a* (shallow)", &rec_a_values, false);
        let l_a = test_parser("Loop", true, "a* (shallow)", &loop_a_values, false);
        
        println!("\n");
        
        println!("--- Deep Expression (a·a·a...·a) ---");
        let r_d = test_parser("Recursive", false, "Deep expression (a·a·a)", &rec_deep_values, true);
        let l_d = test_parser("Loop", true, "Deep expression (a·a·a)", &loop_deep_values, true);
        
        rec_a = r_a;
        loop_a = l_a;
        rec_deep = r_d;
        loop_deep = l_d;
        
    } else {
        // =============================================================
        // RELEASE MODE 
        // =============================================================
        
        // Recursive a*: 10000 to 20000 in steps of 5000
        let rec_a_values = [10000, 15000, 20000];
        
        // Loop a*: 10000 to 50000 in steps of 10000
        let loop_a_values = [10000, 15000, 20000, 30000, 40000, 50000];
        
        // Recursive deep: 10000 to 20000 in steps of 5000
        let rec_deep_values = [10000, 15000, 20000];
        
        // Loop deep: 10000 to 50000 in steps of 1000₀
        let loop_deep_values = [10000, 15000, 20000, 30000, 40000, 50000]; 
        
        println!("--- Shallow Expression (a*) ---");
        let r_a = test_parser("Recursive", false, "a* (shallow)", &rec_a_values, false);
        let l_a = test_parser("Loop", true, "a* (shallow)", &loop_a_values, false);
        
        println!("\n");
        
        println!("--- Deep Expression (a·a·a...·a) ---");
        let r_d = test_parser("Recursive", false, "Deep expression (a·a·a)", &rec_deep_values, true);
        let l_d = test_parser("Loop", true, "Deep expression (a·a·a)", &loop_deep_values, true);
        
        rec_a = r_a;
        loop_a = l_a;
        rec_deep = r_d;
        loop_deep = l_d;
    }
    
    println!();
    println!("================================================================================");
    println!("SUMMARY - {} MODE", mode);
    println!("================================================================================");
    println!();
    println!("  {:<25} | {:>12} | {:>12}", "Test", "Recursive", "Loop");
    println!("  {:<25} | {:>12} | {:>12}", "-----------------------", "------------", "------------");
    println!("  {:<25} | {:>11} chars | {:>11} chars", "a* (shallow)", rec_a, loop_a);
    println!("  {:<25} | {:>11} depth | {:>11} depth", "Deep expression", rec_deep, loop_deep);
    println!();
    
    if rec_a > 0 && loop_a > 0 && loop_a > rec_a {
        let ratio = loop_a as f64 / rec_a as f64;
        println!("  Loop survives {:.2}x longer for a*", ratio);
    }
    
    if rec_deep > 0 && loop_deep > 0 && loop_deep > rec_deep {
        let ratio = loop_deep as f64 / rec_deep as f64;
        println!("  Loop survives {:.2}x deeper expressions", ratio);
    }
    println!("================================================================================");
}