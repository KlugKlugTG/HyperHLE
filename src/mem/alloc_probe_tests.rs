#[cfg(test)]
mod probe_tests {
    use super::Allocator;
    #[test]
    fn probe() {
        let mut a = Allocator::new();
        let base = a.alloc(16);
        eprintln!("PROBE base = {:#x}", base);
    }
}
