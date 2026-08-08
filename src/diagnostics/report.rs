//! regex-engine/src/diagnostics/report.rs
//! 
//! ReportWriter: buffers diagnostic output and writes to a file or stdout.
//!
//! Used exclusively by Level 3. Levels 0–2 print directly to stdout.

const SEPARATOR: &str = "-------------------------------====";

pub struct ReportWriter {
    lines:       Vec<String>,
    output_path: Option<String>,
}

impl ReportWriter {
    /// Create a writer. If path is Some, output goes to that file; else stdout.
    pub fn new(path: Option<&str>) -> Self {
        Self {
            lines:       Vec::new(),
            output_path: path.map(|s| s.to_string()),
        }
    }

    /// Append line of content.
    pub fn line(&mut self, s: &str) {
        self.lines.push(s.to_string());
    }

    /// Append blank line.
    pub fn blank(&mut self) {
        self.lines.push(String::new());
    }

    /// Append standard separator line
    pub fn separator(&mut self) {
        self.lines.push(SEPARATOR.to_string());
    }

    /// Append a key-value line.
    /// Key is left-padded to 20 chars so values align cleanly.
    pub fn kv(&mut self, key: &str, value: &str) {
        // "Failure at position:" is 20 chars - the longest key we use
        self.lines.push(format!("{:<22}{}", format!("{}:", key), value));
    }

    /// Write everything to the configured destination.
    pub fn flush(self) {
        let content = self.lines.join("\n") + "\n";

        match self.output_path {
            None => {
                print!("{}", content);
            }
            Some(ref path) => {
                match std::fs::write(path, &content) {
                    Ok(_)  => eprintln!("[diagnostics] Report written to {}", path),
                    Err(e) => {
                        eprintln!("[diagnostics] Could not write to {}: {}", path, e);
                        eprintln!("[diagnostics] Falling back to stdout.");
                        print!("{}", content);
                    }
                }
            }
        }
    }
}