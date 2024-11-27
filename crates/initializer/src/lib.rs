use std::collections::{HashMap, HashSet};
use std::fs;
use wg_2024::config::Config;
use wg_2024::network::NodeId;

pub fn start(path: &str) -> Result<(), &'static str> {
    let config: Config = parse_config(path)?;
    if !is_connected(&config) {
        Err("Invalid Graph: node not strongly connected")
    } else {
        // TODO: implement the rest
        Ok(())
    }
}

fn parse_config(path: &str) -> Result<Config, &'static str> {
    let config_data = fs::read_to_string(path).map_err(|_| "Unable to read config file")?;
    let config: Config = toml::from_str(&config_data).map_err(|_| "Unable to parse config file")?;
    Ok(config)
}

fn is_connected(config: &Config) -> bool {
    // create a single vec of ids and unify the three adj list
    let mut all_node_ids: HashSet<NodeId> = HashSet::new();
    let mut node_connections: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
    for drone in &config.drone {
        all_node_ids.insert(drone.id);
        all_node_ids.extend(&drone.connected_node_ids);

        let connections = node_connections.entry(drone.id).or_default();
        connections.extend(&drone.connected_node_ids);
    }
    for client in &config.client {
        all_node_ids.insert(client.id);
        all_node_ids.extend(&client.connected_drone_ids);

        let connections = node_connections.entry(client.id).or_default();
        connections.extend(&client.connected_drone_ids);
    }
    for server in &config.server {
        all_node_ids.insert(server.id);
        all_node_ids.extend(&server.connected_drone_ids);

        let connections = node_connections.entry(server.id).or_default();
        connections.extend(&server.connected_drone_ids);
    }

    // if is empty end
    if all_node_ids.is_empty() {
        return true;
    }

    // dfs travel the graph from a random node, if visited eq nodes' set the graph is connected and bilateral
    let start_node = all_node_ids.iter().next().cloned().unwrap();

    let mut visited = HashSet::new();
    let mut to_visit = vec![start_node];

    while let Some(current) = to_visit.pop() {
        if visited.insert(current) {
            // Aggiungi vicini non ancora visitati
            if let Some(node_connections) = node_connections.get(&current) {
                for &neighbor in node_connections {
                    if !visited.contains(&neighbor) {
                        to_visit.push(neighbor);
                    }
                }
            }
        }
    }

    // Controlla se tutti i nodi sono stati visitati
    visited == all_node_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use wg_2024::config::{Client, Drone};

    //TODO: complete TEST after they approve the PartialEq
    #[test]
    fn parse_test() {
        const FILE_CORRECT: &str = "src/test_data/input1.toml";
        // const FILE_INVALID: &str = "src/test_data/input2.toml";
        // const FILE_EMPTY: &str = "src/test_data/input3.toml";
        // test correct file
        let result = parse_config(FILE_CORRECT);
        assert!(result.is_ok(), "Failed to parse the config file");
        let config = result.unwrap();
        assert_eq!(config.drone.len(), 3);
        assert_eq!(config.drone[0].id, 1);
        assert_eq!(config.drone[0].connected_node_ids, vec![2, 3, 5]);
        assert_eq!(config.drone[0].pdr, 0.05);
        assert_eq!(config.drone[1].id, 2);
        assert_eq!(config.drone[1].connected_node_ids, vec![1, 3, 4]);
        assert_eq!(config.drone[1].pdr, 0.03);
        assert_eq!(config.drone[2].id, 3);
        assert_eq!(config.drone[2].connected_node_ids, vec![2, 1, 4]);
        assert_eq!(config.drone[2].pdr, 0.14);
        assert_eq!(config.client.len(), 2);
        assert_eq!(config.client[0].id, 4);
        assert_eq!(config.client[0].connected_drone_ids, vec![3, 2]);
        assert_eq!(config.client[1].id, 5);
        assert_eq!(config.client[1].connected_drone_ids, vec![1]);
        assert_eq!(config.server.len(), 1);
        assert_eq!(config.server[0].id, 6);
        assert_eq!(config.server[0].connected_drone_ids, vec![2, 3]);

        //TODO: parse empty and invalid file
    }

    #[test]
    fn test_is_connected_empty_graph() {
        let config = Config {
            drone: vec![],
            client: vec![],
            server: vec![],
        };
        assert!(
            is_connected(&config),
            "Empty graph should be considered connected."
        );
    }

    #[test]
    fn test_is_connected_single_node() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![],
                pdr: 0.1,
            }],
            client: vec![],
            server: vec![],
        };
        assert!(
            is_connected(&config),
            "Single-node graph should be considered connected."
        );
    }

    #[test]
    fn test_is_connected_connected_graph() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![2],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![3],
                    pdr: 0.1,
                },
                Drone {
                    id: 3,
                    connected_node_ids: vec![1],
                    pdr: 0.1,
                },
            ],
            client: vec![],
            server: vec![],
        };
        assert!(is_connected(&config), "Graph should be connected.");
    }

    #[test]
    fn test_is_connected_disconnected_graph() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![2],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 3,
                connected_drone_ids: vec![],
            }],
            server: vec![],
        };
        assert!(!is_connected(&config), "Graph should not be connected.");
    }
}
