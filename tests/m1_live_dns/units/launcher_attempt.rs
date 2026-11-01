use super::*;

#[test]
fn attempt_paths_stay_scoped_under_the_control_root() {
    let (a, e, c, g) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), 3);
    for path in [dir(a), stop_path(e, c, g), campaign_path(e, c), run_path(a)] {
        let text = path.to_str().unwrap();
        assert!(text.starts_with("/run/duskweave/"), "{text}");
        assert!(!text.contains(".."), "{text}");
    }
    assert!(
        stop_path(e, c, g)
            .to_str()
            .unwrap()
            .contains(&g.to_string())
    );
    assert!(stop_path(e, c, g) != stop_path(e, c, g + 1));
}
