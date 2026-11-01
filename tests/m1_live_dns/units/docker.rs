    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    pub(super) fn root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "dw-docker-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    fn failed_create(result: Res<String>) -> Vec<String> {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let recorded = calls.clone();
        command::inject(move |args, _| {
            recorded.borrow_mut().push(args[0].to_string());
            match args[0] {
                "ps" => Ok(String::new()),
                "create" => result.clone(),
                _ => Err(Fail::Unresolved("live_disposal_unconfirmed")),
            }
        });
        assert!(
            create(
                "sha256:expected",
                Uuid::from_u128(1),
                false,
                Instant::now() + Duration::from_millis(5000)
            )
            .is_err()
        );
        calls.borrow().clone()
    }
    #[test]
    fn create_timeout_is_reconciled_instead_of_losing_teardown_responsibility() {
        let calls = failed_create(Err(Fail::Unresolved("live_host_metadata_unavailable")));
        assert_eq!(
            calls.iter().filter(|c| *c == "ps").count(),
            2,
            "a client timeout must reconcile the exact attempt even when one lookup is absent"
        );
    }
    #[test]
    fn invalid_create_id_is_reconciled_before_returning_failure() {
        let calls = failed_create(Ok("invalid".into()));
        assert_eq!(
            calls.iter().filter(|c| *c == "ps").count(),
            2,
            "a malformed Docker ID must not bypass teardown"
        );
    }
    #[test]
    fn attempt_label_alone_cannot_authorize_removing_a_collision() {
        std::fs::write(record_path(Uuid::from_u128(1), false), "sha256:expected").unwrap();
        let removed = Rc::new(RefCell::new(false));
        let called = removed.clone();
        command::inject(move |args, _| match args[0] {
            "ps" => Ok("present".into()),
            "inspect" => Ok(Uuid::from_u128(1).to_string()),
            "rm" => {
                *called.borrow_mut() = true;
                Ok(String::new())
            }
            _ => unreachable!(),
        });
        assert!(
            remove(
                Uuid::from_u128(1),
                false,
                Instant::now() + Duration::from_millis(1000)
            )
            .is_err()
        );
        assert!(
            !*removed.borrow(),
            "deployment label AND image must agree before removal"
        );
    }
