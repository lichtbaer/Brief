//! Physical RAM detection for model defaults (macOS: sysctl; Linux: /proc/meminfo for dev/CI).

/// Returns installed system memory in gigabytes (binary GB). Falls back to 16.0 if unknown.
pub fn get_available_memory_gb() -> f64 {
    memsize_bytes()
        .map(|b| b as f64 / 1_073_741_824.0)
        .unwrap_or(16.0)
}

/// Ollama model id recommended for the given RAM (unified memory on Apple Silicon).
/// Thresholds: ≤8 GB → 3B param model; ≤16 GB → 8B; ≤32 GB → 8B (sweet spot for most hardware);
/// >32 GB → 70B model for users with high-end workstations or large-RAM laptops.
pub fn recommended_llm_model(ram_gb: f64) -> &'static str {
    if ram_gb <= 8.0 {
        "llama3.2:3b"
    } else if ram_gb <= 32.0 {
        "llama3.1:8b"
    } else {
        // >32 GB: recommend a larger model that fits comfortably in VRAM/unified memory.
        "llama3.3:70b"
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

    #[test]
    fn recommended_zero_ram_uses_small_model() {
        assert_eq!(recommended_llm_model(0.0), "llama3.2:3b");
    }

    #[test]
    fn recommended_very_high_ram_uses_70b() {
        // >32 GB systems get the 70B model recommendation.
        assert_eq!(recommended_llm_model(64.0), "llama3.3:70b");
        assert_eq!(recommended_llm_model(128.0), "llama3.3:70b");
    }

    #[test]
    fn recommended_32gb_uses_8b() {
        // 32 GB is still the 8B tier — the 70B threshold starts strictly above 32 GB.
        assert_eq!(recommended_llm_model(32.0), "llama3.1:8b");
        assert_eq!(recommended_llm_model(32.1), "llama3.3:70b");
    }
}
