use super::{INSTANT, QUICK, SCENE, SETTLE};

#[test]
fn interface_durations_are_strictly_ordered() {
    assert!(INSTANT < QUICK && QUICK < SETTLE && SETTLE < SCENE);
}
