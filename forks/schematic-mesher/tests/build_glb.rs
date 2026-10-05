//! The build GLB exporter: named group nodes, TRS tracks, anchor children,
//! per-node extras, and one material per atlas.
use schematic_mesher::{
    export_build_glb, mesh_from_layers, BuildChild, BuildNode, BuildScene, Interpolation, Mesh,
    MeshLayer, TextureAtlas, Track, Vertex,
};

fn quad_mesh() -> Mesh {
    let mut mesh = Mesh::new();
    let a = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
    let b = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
    let c = mesh.add_vertex(Vertex::new([1.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 1.0]));
    let d = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
    mesh.add_quad(a, b, c, d);
    mesh
}

/// Split a GLB into its parsed JSON chunk and the binary chunk length.
fn glb_json(glb: &[u8]) -> (serde_json::Value, usize) {
    assert_eq!(&glb[0..4], b"glTF");
    assert_eq!(u32::from_le_bytes(glb[4..8].try_into().unwrap()), 2);
    assert_eq!(
        u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize,
        glb.len()
    );
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    assert_eq!(&glb[16..20], b"JSON");
    let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
    let bin_len =
        u32::from_le_bytes(glb[20 + json_len..24 + json_len].try_into().unwrap()) as usize;
    (json, bin_len)
}

#[test]
fn exports_named_nodes_tracks_anchors_and_extras() {
    let scene = BuildScene {
        name: "beacon".into(),
        atlases: vec![TextureAtlas::empty()],
        extras: Some(serde_json::json!({ "nucleation": { "version": 1 } })),
        nodes: vec![
            BuildNode {
                name: "group:0".into(),
                mesh: quad_mesh(),
                atlas: 0,
                translation: Some(Track {
                    times: vec![0.0, 0.5],
                    values: vec![[0.0, 4.0, 0.0], [0.0, 0.0, 0.0]],
                    interpolation: Interpolation::Linear,
                }),
                rotation: Some(Track {
                    times: vec![0.0, 0.5],
                    values: vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.7071068, 0.0, 0.7071068]],
                    interpolation: Interpolation::Linear,
                }),
                scale: Some(Track {
                    times: vec![0.0, 0.5],
                    values: vec![[0.0; 3], [1.0; 3]],
                    interpolation: Interpolation::Step,
                }),
                extras: Some(serde_json::json!({ "nucleation": { "group": 0 } })),
                children: vec![BuildChild {
                    name: "anchor:top".into(),
                    translation: [0.5, 1.0, 0.5],
                    extras: None,
                }],
            },
            BuildNode {
                name: "group:1".into(),
                mesh: Mesh::new(),
                atlas: 0,
                translation: None,
                rotation: None,
                scale: None,
                extras: None,
                children: vec![],
            },
        ],
    };
    let glb = export_build_glb(&scene).unwrap();
    let (json, bin_len) = glb_json(&glb);
    assert!(bin_len > 0);

    let nodes = json["nodes"].as_array().unwrap();
    let names: Vec<&str> = nodes.iter().map(|n| n["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["build:beacon", "group:0", "anchor:top", "group:1"]);
    assert_eq!(nodes[0]["extras"]["nucleation"]["version"], 1);
    assert_eq!(nodes[0]["children"], serde_json::json!([1, 3]));
    assert_eq!(nodes[1]["children"], serde_json::json!([2]));
    assert_eq!(nodes[1]["extras"]["nucleation"]["group"], 0);
    assert_eq!(
        nodes[1]["scale"],
        serde_json::json!([0.0, 0.0, 0.0]),
        "rests at its first key"
    );
    assert_eq!(nodes[2]["translation"], serde_json::json!([0.5, 1.0, 0.5]));
    assert!(nodes[1]["mesh"].is_number(), "group:0 carries a mesh");
    assert!(
        nodes[3]["mesh"].is_null(),
        "an empty group is an empty node"
    );

    let animation = &json["animations"][0];
    assert_eq!(animation["name"], "beacon");
    let channels = animation["channels"].as_array().unwrap();
    assert_eq!(channels.len(), 3);
    let paths: Vec<&str> = channels
        .iter()
        .map(|c| c["target"]["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["translation", "rotation", "scale"]);
    assert!(channels.iter().all(|c| c["target"]["node"] == 1));
    let interpolations: Vec<&str> = animation["samplers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["interpolation"].as_str().unwrap())
        .collect();
    assert_eq!(interpolations, ["LINEAR", "LINEAR", "STEP"]);
    let rotation_output = animation["samplers"][1]["output"].as_u64().unwrap() as usize;
    assert_eq!(json["accessors"][rotation_output]["type"], "VEC4");

    assert_eq!(json["materials"].as_array().unwrap().len(), 1);
    assert_eq!(json["meshes"][0]["primitives"][0]["material"], 0);
    assert_eq!(json["scenes"][0]["nodes"], serde_json::json!([0]));
}

#[test]
fn one_material_per_atlas() {
    let scene = BuildScene {
        name: "two".into(),
        atlases: vec![TextureAtlas::empty(), TextureAtlas::empty()],
        extras: None,
        nodes: vec![
            BuildNode {
                name: "group:0".into(),
                mesh: quad_mesh(),
                atlas: 0,
                translation: None,
                rotation: None,
                scale: None,
                extras: None,
                children: vec![],
            },
            BuildNode {
                name: "group:1".into(),
                mesh: quad_mesh(),
                atlas: 1,
                translation: None,
                rotation: None,
                scale: None,
                extras: None,
                children: vec![],
            },
        ],
    };
    let (json, _) = glb_json(&export_build_glb(&scene).unwrap());
    assert_eq!(json["materials"].as_array().unwrap().len(), 2);
    assert_eq!(json["images"].as_array().unwrap().len(), 2);
    assert_eq!(json["meshes"][1]["primitives"][0]["material"], 1);
    assert!(json["animations"].as_array().map_or(true, |a| a.is_empty()));
}

#[test]
fn mesh_from_layers_offsets_indices() {
    let layer = MeshLayer {
        positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        colors: vec![[1.0; 4]; 3],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let mesh = mesh_from_layers(&[&layer, &layer]);
    assert_eq!(mesh.vertices.len(), 6);
    assert_eq!(mesh.indices, vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn rejects_a_scene_with_no_geometry() {
    let scene = BuildScene {
        name: "empty".into(),
        atlases: vec![],
        nodes: vec![],
        extras: None,
    };
    assert!(export_build_glb(&scene).is_err());
    let scene = BuildScene {
        name: "bad-atlas".into(),
        atlases: vec![],
        extras: None,
        nodes: vec![BuildNode {
            name: "group:0".into(),
            mesh: quad_mesh(),
            atlas: 0,
            translation: None,
            rotation: None,
            scale: None,
            extras: None,
            children: vec![],
        }],
    };
    assert!(export_build_glb(&scene).is_err());
}
