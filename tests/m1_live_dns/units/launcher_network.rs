    use super::*;
    #[test]
    #[ignore = "requires the packet-owned Linux host and namespace privileges"]
    fn owned_lab_namespace_can_be_prepared_without_transmitting() {
        assert_eq!(rustix::process::geteuid().as_raw(), 0);
        let attempt = Uuid::parse_str(
            std::fs::read_to_string("/proc/sys/kernel/random/uuid")
                .unwrap()
                .trim(),
        )
        .unwrap();
        let image = super::super::docker::metadata(&[
            "image",
            "inspect",
            "--format",
            "{{.Id}}",
            "dw-live-dns-20261030:candidate",
        ])
        .unwrap();
        let preparation_start = Instant::now();
        let end = preparation_start + Duration::from_millis(5000);
        let container = super::super::docker::create(image.trim(), attempt, false, end).unwrap();
        let prepared = create(attempt, container.pid, "192.0.2.53", end);
        let elapsed = preparation_start.elapsed();
        let cleanup_end = Instant::now() + Duration::from_millis(1000);
        let cleaned = destroy(attempt, cleanup_end);
        super::super::docker::remove(attempt, false, cleanup_end).unwrap();
        eprintln!(
            "preparation_ms={} egress_granted=false",
            elapsed.as_millis()
        );
        assert!(
            prepared.is_ok(),
            "owned namespace preparation failed: {prepared:?}"
        );
        assert!(cleaned);
        assert!(!std::path::Path::new(&netns_path(attempt)).exists());
    }
