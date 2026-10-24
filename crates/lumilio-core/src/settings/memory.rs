/// The machine's installed memory in MB, if the system will say. Best effort:
/// `None` means "unknown", never "none".
#[must_use]
pub fn total_memory_mb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb / 1024)
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        let bytes: u64 = String::from_utf8(output.stdout).ok()?.trim().parse().ok()?;
        Some(bytes / (1024 * 1024))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// A sensible largest heap for the game on a machine with `total_mb`: half
/// the memory, at most 8 GB, at least 2 GB (or everything on a small
/// machine), in steps of 512 MB.
#[must_use]
pub fn recommended_memory_mb(total_mb: u64) -> u64 {
    let half = (total_mb / 2).min(8192);
    let stepped = half / 512 * 512;
    stepped.max(2048.min(total_mb))
}

#[cfg(test)]
mod tests;
