//! Physical RAM detection for model defaults (macOS: sysctl; Linux: /proc/meminfo for dev/CI).

/// Returns installed system memory in gigabytes (binary GB). Falls back to 16.0 if unknown.
pub fn get_available_memory_gb() -> f64 {
    memsize_bytes()
        .map(|b| b as f64 / 1_073_741_824.0)
        .unwrap_or(16.0)
}

/// Ollama model id recommended for the given RAM (unified memory on Apple Silicon).
pub fn recommended_llm_model(ram_gb: f64) -> &'static str {
    if ram_gb <= 8.0 {
        "llama3.2:3b"
    } else if ram_gb <= 16.0 {
        "llama3.1:8b"
    } else {
        "llama3.1:8b"
    }
}

#[cfg(target_os = "macos")]
fn memsize_bytes() -> Option<u64> {
    let output = std::process::Command::new("sysctl")
        .arg("-n")
        .arg("hw.memsize")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

#[cfg(target_os = "linux")]
fn memsize_bytes() -> Option<u64> {
    let content = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb = rest.split_whitespace().next()?.parse::<u64>().ok()?;
            return Some(kb * 1024);
        }
    }
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn memsize_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommended_up_to_8gb_uses_3b() {
        assert_eq!(recommended_llm_model(8.0), "llama3.2:3b");
        assert_eq!(recommended_llm_model(4.0), "llama3.2:3b");
    }

    #[test]
    fn recommended_above_8gb_uses_8b() {
        assert_eq!(recommended_llm_model(8.1), "llama3.1:8b");
        assert_eq!(recommended_llm_model(16.0), "llama3.1:8b");
        assert_eq!(recommended_llm_model(32.0), "llama3.1:8b");
    }
}
