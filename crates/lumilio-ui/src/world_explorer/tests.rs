use super::camera::Camera;
#[test]
fn zoom_keeps_pointer_world_coordinate_and_lod_uses_fourfold_steps() {
    let mut camera = Camera::default();
    let before = camera.world([100., 50.], [800, 600]);
    camera.zoom(0.5, [100., 50.], [800, 600]);
    assert_eq!(camera.world([100., 50.], [800, 600]), before);
    assert_eq!(camera.level(), 0);
    camera.scale = 16.;
    assert_eq!(camera.level(), 2);
}
