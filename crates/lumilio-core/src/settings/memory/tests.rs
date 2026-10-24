use super::recommended_memory_mb;

#[test]
fn the_recommendation_is_half_the_memory_within_bounds() {
    assert_eq!(recommended_memory_mb(16_384), 8192);
    assert_eq!(recommended_memory_mb(65_536), 8192);
    assert_eq!(recommended_memory_mb(8192), 4096);
    assert_eq!(recommended_memory_mb(6000), 2560);
    assert_eq!(recommended_memory_mb(4096), 2048);
    assert_eq!(
        recommended_memory_mb(1024),
        1024,
        "everything on a tiny machine"
    );
}
