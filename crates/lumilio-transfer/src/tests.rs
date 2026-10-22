use std::time::Duration;

use super::RetryPolicy;

#[test]
fn retry_delay_is_exponential_and_capped() {
    let policy =
        RetryPolicy::fixed(6, Duration::from_millis(100), Duration::from_millis(350)).unwrap();

    assert_eq!(policy.delay_before_attempt(1), Duration::ZERO);
    assert_eq!(policy.delay_before_attempt(2), Duration::from_millis(100));
    assert_eq!(policy.delay_before_attempt(3), Duration::from_millis(200));
    assert_eq!(policy.delay_before_attempt(4), Duration::from_millis(350));
}
