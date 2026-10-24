use super::{bayer4, fbm2, hash2, smoothstep, value1, value2};

#[test]
fn hashes_are_stable_bounded_and_seed_sensitive() {
    assert_eq!(hash2(3, -7, 11), hash2(3, -7, 11));
    assert_ne!(hash2(3, -7, 11), hash2(3, -7, 12));
    for x in -40..40 {
        for y in -40..40 {
            let value = hash2(x, y, 5);
            assert!((0. ..1.).contains(&value));
        }
    }
}

#[test]
fn value_noise_is_continuous_between_lattice_points() {
    for step in 0..200 {
        let x = step as f32 * 0.05;
        assert!((value1(x, 1) - value1(x + 0.01, 1)).abs() < 0.05);
        assert!((value2(x, 1.3, 2) - value2(x + 0.01, 1.3, 2)).abs() < 0.05);
        assert!((0. ..1.).contains(&fbm2(x, x * 0.5, 3)));
    }
}

#[test]
fn ordered_dither_covers_every_threshold_once() {
    let mut values: Vec<f32> = (0..4)
        .flat_map(|y| (0..4).map(move |x| bayer4(x, y)))
        .collect();
    values.sort_by(f32::total_cmp);
    values.dedup();
    assert_eq!(values.len(), 16);
    assert_eq!(bayer4(-1, -1), bayer4(3, 3));
}

#[test]
fn smoothstep_clamps_outside_its_edges() {
    assert_eq!(smoothstep(1., 2., 0.), 0.);
    assert_eq!(smoothstep(1., 2., 3.), 1.);
    assert!((smoothstep(1., 2., 1.5) - 0.5).abs() < 1e-6);
}
