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
        let cid = 1;
        state.add_node(cid, NodeType::Client);
        assert!(!state.id_to_idx.contains_key(&cid));
        assert!(!state.server_list.contains(&cid));

        let sid = 2;
        state.add_node(sid, NodeType::Server);
        assert!(state.id_to_idx.contains_key(&sid));
        assert!(state.server_list.contains(&sid));

        let nid = 3;
        state.add_node(nid, NodeType::Drone);
        assert!(state.id_to_idx.contains_key(&nid));
        assert!(!state.server_list.contains(&nid));

        state.remove_node(&cid);
        assert!(!state.id_to_idx.contains_key(&cid));

        state.remove_node(&sid);
        assert!(state.id_to_idx.contains_key(&sid));

        state.remove_node(&sid);
        assert!(state.id_to_idx.contains_key(&sid));
    }

    #[test]
    fn test_add_link_creates_nodes() {
        let mut state = setup_state();
        let my_id = state.topology[state.start_idx];
        let drone_a = 1;
        let drone_b = 2;
        let server_c = 3;

        // Configuration : Client <-> Drone A <-> Drone B -> Server C
        state.add_link(my_id, drone_a, NodeType::Client, NodeType::Drone, 5);
        state.add_link(drone_a, drone_b, NodeType::Drone, NodeType::Drone, 10);
        state.add_link(drone_b, server_c, NodeType::Drone, NodeType::Server, 8);

        let idx_client = state.start_idx;
        let idx_a = state.id_to_idx[&drone_a];
        let idx_b = state.id_to_idx[&drone_b];
        let idx_c = state.id_to_idx[&server_c];

        assert_eq!(state.topology.find_edge(idx_c, idx_b), None);

        let edge_client_a = state.topology.find_edge(idx_client, idx_a).unwrap();
        let edge_a_client = state.topology.find_edge(idx_a, idx_client).unwrap();
        assert_eq!(state.topology.edge_weight(edge_client_a), Some(&5));
        assert_eq!(state.topology.edge_weight(edge_a_client), Some(&5));

        let edge_a_b = state.topology.find_edge(idx_a, idx_b).unwrap();
        let edge_b_a = state.topology.find_edge(idx_b, idx_a).unwrap();
        assert_eq!(state.topology.edge_weight(edge_a_b), Some(&10));
        assert_eq!(state.topology.edge_weight(edge_b_a), Some(&10));

        let edge_b_c = state.topology.find_edge(idx_b, idx_c).unwrap();
        assert_eq!(state.topology.edge_weight(edge_b_c), Some(&8));

        state = setup_state();
        let mut a = 100;
        let mut b = 101;
        state.add_link(a, b, NodeType::Server, NodeType::Drone, 5);
        let idx_a = state.id_to_idx[&a];
        let idx_b = state.id_to_idx[&b];
        assert_eq!(state.topology.find_edge(idx_a, idx_b), None);
        let edge_b_a = state.topology.find_edge(idx_b, idx_a).unwrap();
        assert_eq!(state.topology.edge_weight(edge_b_a), Some(&5));
        assert!(state.id_to_idx.contains_key(&a));
        assert!(state.id_to_idx.contains_key(&b));
        assert!(state.server_list.contains(&a));
        assert!(!state.server_list.contains(&b));

        a = 1;
        b = 3;
        state.add_link(a, b, NodeType::Drone, NodeType::Client, 5);
        assert!(!state.id_to_idx.contains_key(&b));
        assert!(!state.server_list.contains(&a));
        assert!(!state.server_list.contains(&b));
    }

    #[test]
    fn test_increment_weight_around_node() {
        let mut state = setup_state();
        let my_id = state.topology[state.start_idx];
        let drone_a = 1;
        let drone_b = 2;
        let server_c = 3;

        // Configuration : Client <-> Drone A <-> Drone B -> Server C
        state.add_link(my_id, drone_a, NodeType::Client, NodeType::Drone, 5);
        state.add_link(drone_a, drone_b, NodeType::Drone, NodeType::Drone, 10);
        state.add_link(drone_b, server_c, NodeType::Drone, NodeType::Server, 8);

        let idx_client = state.start_idx;
        let idx_a = state.id_to_idx[&drone_a];
        let idx_b = state.id_to_idx[&drone_b];
        let idx_c = state.id_to_idx[&server_c];

        let edge_client_a = state.topology.find_edge(idx_client, idx_a).unwrap();
        let edge_a_client = state.topology.find_edge(idx_a, idx_client).unwrap();
        let edge_a_b = state.topology.find_edge(idx_a, idx_b).unwrap();
        let edge_b_a = state.topology.find_edge(idx_b, idx_a).unwrap();
        let edge_b_c = state.topology.find_edge(idx_b, idx_c).unwrap();

        assert_eq!(state.topology.find_edge(idx_c, idx_b), None);

        state.increment_weight_around_node(&drone_a, 4);

        assert_eq!(state.topology.edge_weight(edge_client_a), Some(&9));
        assert_eq!(state.topology.edge_weight(edge_a_client), Some(&9));
        assert_eq!(state.topology.edge_weight(edge_a_b), Some(&14));
        assert_eq!(state.topology.edge_weight(edge_b_a), Some(&14));
        assert_eq!(state.topology.edge_weight(edge_b_c), Some(&8));

        state.increment_weight_around_node(&drone_a, -10);

        assert_eq!(state.topology.edge_weight(edge_client_a), Some(&1));
        assert_eq!(state.topology.edge_weight(edge_a_client), Some(&1));
        assert_eq!(state.topology.edge_weight(edge_a_b), Some(&4));
        assert_eq!(state.topology.edge_weight(edge_b_a), Some(&4));
        assert_eq!(state.topology.edge_weight(edge_b_c), Some(&8));

        state.increment_weight_around_node(&server_c, 2);

        assert_eq!(state.topology.edge_weight(edge_b_c), Some(&10));
        assert_eq!(state.topology.edge_weight(edge_client_a), Some(&1));
        assert_eq!(state.topology.edge_weight(edge_a_client), Some(&1));
        assert_eq!(state.topology.edge_weight(edge_a_b), Some(&4));
        assert_eq!(state.topology.edge_weight(edge_b_a), Some(&4));
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
        state.add_link(0, a, NodeType::Client, NodeType::Drone, 1);
        state.add_link(0, b, NodeType::Client, NodeType::Drone, 1);
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
        state.add_link(0, a, NodeType::Client, NodeType::Drone, 1);
        state.add_link(0, b, NodeType::Client, NodeType::Drone, 10);
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
        let nid = 1;
        state.add_link(0, nid, NodeType::Client, NodeType::Drone, 1);
        state.add_link(nid, sid, NodeType::Drone, NodeType::Server, 1);

        let expected_path = vec![0, 1, 100];

        let result_path = state.get_server_path(&sid).unwrap();
        assert_eq!(result_path, expected_path);
    }

    #[test]
    fn test_get_server_path_shortest_with_server_middle() {
        let mut state = setup_state();
        let sid_a = 100;
        let sid_b = 101;
        let drone_a = 1;
        let drone_b = 2;
        let drone_c = 3;
        let drone_d = 4;
        state.add_link(0, drone_a, NodeType::Client, NodeType::Drone, 1);
        state.add_link(drone_a, sid_a, NodeType::Drone, NodeType::Server, 1);
        state.add_link(sid_a, drone_d, NodeType::Server, NodeType::Drone, 1);
        state.add_link(0, drone_b, NodeType::Client, NodeType::Drone, 10);
        state.add_link(drone_b, drone_c, NodeType::Drone, NodeType::Drone, 1);
        state.add_link(drone_c, drone_d, NodeType::Drone, NodeType::Drone, 1);
        state.add_link(drone_d, sid_b, NodeType::Drone, NodeType::Server, 1);

        let expected_path = vec![0, 2, 3, 4, 101];

        let result = state.get_server_path(&sid_b);
        let result_path = result.unwrap();
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
