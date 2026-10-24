use super::{ActivityGraph, ActivityGraphError, ActivityNode};

#[test]
fn graph_rejects_missing_prerequisites_before_execution() {
    let mut graph = ActivityGraph::new();
    graph
        .add(ActivityNode::new("child", "Child", |_| async { Ok(()) }).after("missing-parent"))
        .unwrap();

    assert!(matches!(
        graph.validate(),
        Err(ActivityGraphError::MissingPrerequisite { .. })
    ));
}
