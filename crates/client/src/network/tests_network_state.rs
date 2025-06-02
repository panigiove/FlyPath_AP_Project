#[cfg(test)]
mod tests {
    use crate::network::{NetworkState, NEW_STATE_GRACE_PERIOD};
    use petgraph::algo::dijkstra;
    use std::time::Duration;
    use wg_2024::packet::NodeType;

    fn setup_state() -> NetworkState {
        NetworkState::new(0, Duration::from_secs(1), 100, 100)
    }

    #[test]
    fn test_should_flood_time() {
        let state = setup_state();
        std::thread::sleep(Duration::from_millis(1100));
        assert!(state.should_flood());
    }

    #[test]
    fn test_should_flood_error_count() {
        let mut state = setup_state();
        state.failed_error_count = 200;
        assert!(state.should_flood());
    }

    #[test]
    fn test_should_flood_drop_count() {
        let mut state = setup_state();
        state.failed_drop_count = 200;
        assert!(state.should_flood());
    }

    #[test]
    fn test_should_flood_after_missing() {
        let mut state = setup_state();
        assert!(!state.should_flood_after_missing());

        state.creation_time =
            std::time::SystemTime::now() - NEW_STATE_GRACE_PERIOD - Duration::from_secs(1);
        assert!(state.should_flood_after_missing());
    }

    #[test]
    fn test_add_node_and_remove_node() {
        let mut state = setup_state();
        let nid = 1;
        state.add_node(nid, NodeType::Client);
        assert!(!state.id_to_idx.contains_key(&nid));
        assert!(!state.server_list.contains(&nid));

        let sid = 2;
        let expected = vec![sid];
        state.add_node(sid, NodeType::Server);
        assert!(state.id_to_idx.contains_key(&sid));
        assert!(state.server_list.contains(&sid));

        state.remove_node(&nid);
        assert!(!state.id_to_idx.contains_key(&nid));

        state.remove_node(&sid);
        assert!(state.id_to_idx.contains_key(&sid));
    }

    #[test]
    fn test_add_link_creates_nodes() {
        let mut state = setup_state();
        let mut a = 1;
        let mut b = 2;

        let expected = vec![b];

        state.add_link(a, b, NodeType::Drone, NodeType::Server, 5);
        assert!(state.id_to_idx.contains_key(&a));
        assert!(state.id_to_idx.contains_key(&b));
        assert!(!state.server_list.contains(&a));
        assert!(state.server_list.contains(&b));

        a = 1;
        b = 3;
        state.add_link(a, b, NodeType::Drone, NodeType::Client, 5);
        assert!(!state.id_to_idx.contains_key(&b));
        assert!(!state.server_list.contains(&a));
        assert!(!state.server_list.contains(&b));

        a = 0; // start_id
        b = 3;
        state.add_link(a, b, NodeType::Client, NodeType::Drone, 5);
        assert!(state.id_to_idx.contains_key(&a));
        assert!(state.id_to_idx.contains_key(&b));
        assert!(!state.server_list.contains(&a));
        assert!(!state.server_list.contains(&b));
    }

    #[test]
    fn test_increment_weight_around_node() {
        let mut state = setup_state();
        let a = 1;
        let b = 2;

        state.add_link(a, b, NodeType::Drone, NodeType::Server, 5);
        let idx_a = state.id_to_idx[&a];
        let idx_b = state.id_to_idx[&b];

        let edge = state.topology.find_edge(idx_a, idx_b).unwrap();
        assert_eq!(state.topology.edge_weight(edge), Some(&5));

        state.increment_weight_around_node(&a, 4);
        assert_eq!(state.topology.edge_weight(edge), Some(&9));

        state.increment_weight_around_node(&a, -10);
        assert_eq!(state.topology.edge_weight(edge), Some(&1));
    }

    #[test]
    fn test_reconstruct_path_start_to_self() {
        let mut state = setup_state();
        let idx = state.id_to_idx.get(&0).unwrap();

        let distances = dijkstra(&state.topology, *idx, None, |e| *e.weight());

        let path = state._reconstruct_path(&distances, *idx);
        assert_eq!(path, Some(vec![0]));
    }

    #[test]
    fn test_reconstruct_path_unreachable_node() {
        let mut state = setup_state();
        let a = 1;
        state.add_node(a, NodeType::Drone);
        let idx = state.id_to_idx.get(&a).unwrap();

        let distances = dijkstra(&state.topology, *idx, None, |e| *e.weight());

        let path = state._reconstruct_path(&distances, *idx);
        assert_eq!(path, None);
    }

    #[test]
    fn test_reconstruct_path_cycle_detection() {
        let mut state = setup_state();
        let a = 1;
        let b = 2;
        state.add_link(0, a, NodeType::Drone, NodeType::Drone, 1);
        state.add_link(0, b, NodeType::Drone, NodeType::Drone, 1);
        state.add_link(a, b, NodeType::Drone, NodeType::Drone, 1);
        let ax = state.id_to_idx.get(&a).unwrap();
        let bx = state.id_to_idx.get(&b).unwrap();

        let distances = dijkstra(&state.topology, state.start_idx, None, |e| *e.weight());

        let path = state._reconstruct_path(&distances, *bx);
        assert_eq!(path, Some(vec![0, 2]));
    }

    #[test]
    fn test_reconstruct_path_simple() {
        let mut state = setup_state();
        let a = 1;
        let b = 2;
        state.add_link(0, a, NodeType::Drone, NodeType::Drone, 1);
        state.add_link(0, b, NodeType::Drone, NodeType::Drone, 10);
        state.add_link(a, b, NodeType::Drone, NodeType::Drone, 1);
        let ax = state.id_to_idx.get(&a).unwrap();
        let bx = state.id_to_idx.get(&b).unwrap();

        let distances = dijkstra(&state.topology, state.start_idx, None, |e| *e.weight());

        let path = state._reconstruct_path(&distances, *bx);
        assert_eq!(path, Some(vec![0, 1, 2]));
    }

    #[test]
    fn test_get_server_path_no_server() {
        let mut state = setup_state();
        let result = state.get_server_path(&100);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_server_path_no_path_to_server() {
        let mut state = setup_state();
        state.add_node(100, NodeType::Server);

        let result = state.get_server_path(&100);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_server_path_compute_new_path_and_cached() {
        let mut state = setup_state();
        let sid = 100;
        state.add_link(0, sid, NodeType::Drone, NodeType::Server, 1);

        let expected_path = vec![0, 100];

        let result = state.get_server_path(&sid);
        if let (result_path) = result.unwrap() {
            assert_eq!(result_path, expected_path);
        } else {
            panic!();
        }

        let result_path = state.get_server_path(&sid).unwrap();
        assert_eq!(result_path, expected_path);
    }

    #[test]
    fn test_recompute_routes_no_servers() {
        let mut state = setup_state();
        let result = state.recompute_all_routes_to_server(None);
        assert!(result);
    }

    #[test]
    fn test_recompute_routes_server_unreachable() {
        let mut state = setup_state();
        state.creation_time =
            std::time::SystemTime::now() - NEW_STATE_GRACE_PERIOD - Duration::from_secs(1);
        let sid = 100;
        state.add_node(sid, NodeType::Server);
        let result = state.recompute_all_routes_to_server(None);
        assert!(!result);
    }

    #[test]
    fn test_recompute_routes_server_path_cached() {
        let mut state = setup_state();
        let sid = 100;
        state.add_link(0, sid, NodeType::Drone, NodeType::Server, 1);
        state.get_server_path(&sid);

        let expected_path = vec![0, 100];

        let result = state.recompute_all_routes_to_server(None);
        assert!(result);
        assert_eq!(*state.routing_table.get(&sid).unwrap(), expected_path);
    }

    #[test]
    fn test_recompute_routes_filtered_by_node() {
        let mut state = setup_state();
        let sid = 100;
        let a = 1;
        let b = 2;
        state.add_link(0, a, NodeType::Drone, NodeType::Drone, 1);
        state.add_link(0, b, NodeType::Drone, NodeType::Drone, 2);
        state.add_link(sid, a, NodeType::Server, NodeType::Drone, 1);
        state.add_link(sid, b, NodeType::Server, NodeType::Drone, 1);
        state.get_server_path(&sid);
        state.increment_weight_around_node(&a, 3);

        let expected_path = vec![0, b, sid];

        let result = state.recompute_all_routes_to_server(Some(&a));
        assert!(result);
        assert_eq!(*state.routing_table.get(&sid).unwrap(), expected_path);
    }
}
