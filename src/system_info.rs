//! Cross-platform system information queries.
//!
//! Uses the `sysinfo` crate for portable CPU and memory detection.
//! All functions are pure queries — no side effects.

use sysinfo::System;

/// Query: number of physical CPU cores available on this machine.
pub fn cpu_count() -> u32 {
    System::new_with_specifics(
        sysinfo::RefreshKind::nothing().with_cpu(sysinfo::CpuRefreshKind::nothing()),
    )
    .cpus()
    .len()
    .max(1) as u32
}

/// Query: total physical memory in MiB.
pub fn total_memory_mib() -> u32 {
    let sys = System::new_with_specifics(
        sysinfo::RefreshKind::nothing().with_memory(sysinfo::MemoryRefreshKind::everything()),
    );
    (sys.total_memory() / (1024 * 1024)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_count_is_positive() {
        assert!(cpu_count() >= 1);
    }

    #[test]
    fn total_memory_is_positive() {
        assert!(total_memory_mib() > 0);
    }
}
