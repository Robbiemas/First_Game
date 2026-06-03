use mole_core::collision::{capsules_intersect_3d, Capsule3, Mat3x4, Vec3};

#[test]
fn capsule_collision_preserves_z_axis_separation() {
    let hit = Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.5);
    let hurt = Capsule3::new(Vec3::new(1.0, 0.0, 1.25), Vec3::new(1.0, 2.0, 1.25), 0.5);

    assert!(!capsules_intersect_3d(&hit, &hurt));
}

#[test]
fn capsule_collision_hits_when_3d_distance_is_inside_combined_radius() {
    let hit = Capsule3::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), 0.75);
    let hurt = Capsule3::new(Vec3::new(1.0, 0.0, 0.9), Vec3::new(1.0, 2.0, 0.9), 0.75);

    assert!(capsules_intersect_3d(&hit, &hurt));
}

#[test]
fn mat3x4_transform_point_keeps_source_z() {
    let matrix = Mat3x4::from_rows([
        [1.0, 0.0, 0.0, 10.0],
        [0.0, 1.0, 0.0, 20.0],
        [0.0, 0.0, 1.0, 30.0],
    ]);

    let point = matrix.transform_point(Vec3::new(1.5, 2.5, 3.5));

    assert_eq!(point, Vec3::new(11.5, 22.5, 33.5));
}
