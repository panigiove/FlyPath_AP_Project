use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use thiserror::Error;
use wg_2024::config::{Config, Drone};
use wg_2024::network::NodeId;
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error reading file: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub fn parse_config<P: AsRef<Path>>(path: P) -> Result<Config, ConfigError> {
    let content = fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&content)?;
    validate(&cfg)?;
    Ok(cfg)
}

//checks if the Network Initialization File meets the needed requirements
fn validate(cfg: &Config) -> Result<(), ConfigError> {

    are_ids_unique(cfg)?;
    
    check_drone_requirement(cfg)?;
    
    check_client_requirements(cfg)?;
    
    check_server_requirements(cfg)?;
    
    if !is_connected(cfg){
        return Err(ConfigError::Validation("The graph is not connected".to_string()))
    }

    if !is_bidirectional(cfg){
        return Err(ConfigError::Validation("The graph is not bidirectional".to_string()))
    }

    if !are_client_server_at_edge(cfg){
        return Err(ConfigError::Validation("Clients and/ or severs aren't at the edge of the network".to_string()))
    }

    Ok(())
}

pub fn start() {
    match parse_config("config.toml") {
        Ok(cfg) => {
            println!("Configurazione valida: {:#?}", cfg);
        }
        Err(e) => {
            eprintln!("Errore nella configurazione: {}", e);
            std::process::exit(1);
        }
    }
}

//the Network Initialization File should represent a connected graph
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

//The Network Initialization File should represent a bidirectional graph
fn is_bidirectional(cfg: &Config) -> bool{
    let mut edges:HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    for drone in &cfg.drone{
        edges.entry(drone.id).or_insert_with(HashSet::new).extend(&drone.connected_node_ids);
    }
    for client in &cfg.client{
        edges.entry(client.id).or_insert_with(HashSet::new).extend(&client.connected_drone_ids);
    }
    for server in &cfg.server{
        edges.entry(server.id).or_insert_with(HashSet::new).extend(&server.connected_drone_ids);
    }
    for (node1, connections1) in &edges{
        for node2 in connections1{
            if let Some(connections2) = edges.get(node2){
                if !connections2.contains(node1){
                    return false
                }
            }
            else{
                return false
            }
        }
    }
    true
}

//the Network Initialization File should represent a network where clients and servers are at the edges of the network
fn are_client_server_at_edge(cfg: &Config) -> bool {
    let drone_ids: std::collections::HashSet<_> = cfg.drone.iter().map(|d| d.id).collect();

    let cleaned_drones = cfg.drone.iter().map(|drone| {
        let filtered_ids = drone
            .connected_node_ids
            .iter()
            .cloned()
            .filter(|id| drone_ids.contains(id))
            .collect();

        Drone {
            id: drone.id,
            connected_node_ids: filtered_ids,
            pdr: drone.pdr,
        }
    }).collect();

    let config_only_drones = Config {
        drone: cleaned_drones,
        client: vec![],
        server: vec![],
    };

    is_connected(&config_only_drones)
}

//The Network Initialization File should never contain two nodes with the same node_id value
fn are_ids_unique(cfg: &Config) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();

    for drone in &cfg.drone {
        if !seen.insert(drone.id) {
            return Err(ConfigError::Validation(format!("The id = [{}] is duplicated", drone.id)));
        }
    }

    for client in &cfg.client {
        if !seen.insert(client.id) {
            return Err(ConfigError::Validation(format!("The id = [{}] is duplicated", client.id)));
        }
    }

    for server in &cfg.server {
        if !seen.insert(server.id) {
            return Err(ConfigError::Validation(format!("The id = [{}] is duplicated", server.id)));
        }
    }

    Ok(())
}

fn check_drone_requirement(cfg: &Config) -> Result<(), ConfigError>{
    let all_ids: HashSet<_> = cfg.drone.iter().map(|d| d.id).collect();
    let client_ids: HashSet<_> = cfg.client.iter().map(|c| c.id).collect();
    let server_ids: HashSet<_> = cfg.server.iter().map(|s| s.id).collect();

    for drone in &cfg.drone {
        let mut seen = HashSet::new();
        for id in &drone.connected_node_ids {
            if !seen.insert(id) {
                return Err(ConfigError::Validation(format!(
                    "The drone with id = [{}] has a duplicate in the connected_node_ids list: [{}]",
                    drone.id, id
                )));
            }
            
            if drone.id == *id {
                return Err(ConfigError::Validation(format!(
                    "The drone with id = [{}] cannot be connected to itself",
                    drone.id
                )));
            }
            
            if !all_ids.contains(id) && !client_ids.contains(id) && !server_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The drone with id = [{}] is connected to an unknown node id = [{}]",
                    drone.id, id
                )));
            }
        }

        // You might want to define a minimum number of connections (example: at least 1)
        // if drone.connected_node_ids.is_empty() {
        //     return Err(ConfigError::Validation(format!(
        //         "The drone with id = [{}] must be connected to at least one node",
        //         drone.id
        //     )));
        // }
    }
    
    Ok(())
}

fn check_client_requirements(cfg: &Config) -> Result<(), ConfigError>{
    
    let drone_ids: HashSet<_> = cfg.drone.iter().map(|d| d.id).collect();
    let client_ids: HashSet<_> = cfg.client.iter().map(|c| c.id).collect();
    let server_ids: HashSet<_> = cfg.server.iter().map(|s| s.id).collect();

    for client in &cfg.client {
        let mut seen_ids = HashSet::new();

        //a client cannot connect to other clients or servers
        for id in &client.connected_drone_ids {
            if client_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The client with id = [{}] cannot be connected to another client (with id = [{}])",
                    client.id, id
                )));
            }

            if server_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The client with id = [{}] cannot be connected to a server (with id = [{}])",
                    client.id, id
                )));
            }
            

            //connected_drone_ids cannot contain repetitions
            if !seen_ids.insert(id) {
                return Err(ConfigError::Validation(format!(
                    "The client with id = [{}] has a duplicate in the drone's list, which is: id = [{}]",
                    client.id, id
                )));
            }

            //checks if the node really exists
            if !drone_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The client with id = [{}] is connected to the id = [{}] which is not valid",
                    client.id, id
                )));
            }
        }

        //a client can be connected to at least one and at most two drones
        let count = client.connected_drone_ids.len();
        if count == 0 || count > 2 {
            return Err(ConfigError::Validation(format!(
                "The client with id = [{}] can be connected to at least one and at most two drones but found: {}",
                client.id, count
            )));
        }
    }

    Ok(())
}

fn check_server_requirements(cfg: &Config) -> Result<(), ConfigError>{
    let drone_ids: HashSet<_> = cfg.drone.iter().map(|d| d.id).collect();
    let client_ids: HashSet<_> = cfg.client.iter().map(|c| c.id).collect();
    let server_ids: HashSet<_> = cfg.server.iter().map(|s| s.id).collect();

    for server in &cfg.server {
        let mut seen_ids = HashSet::new();

        //a server cannot connect to other clients or servers
        for id in &server.connected_drone_ids {
            if client_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The server with id = [{}] cannot be connected to a client (with id = [{}])",
                    server.id, id
                )));
            }

            if server_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The server with id = [{}] cannot be connected to another server (with id = [{}])",
                    server.id, id
                )));
            }


            //connected_drone_ids cannot contain repetitions
            if !seen_ids.insert(id) {
                return Err(ConfigError::Validation(format!(
                    "The server with id = [{}] has a duplicate in the drone's list, which is: id = [{}]",
                    server.id, id
                )));
            }

            //checks if the node really exists
            if !drone_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The server with id = [{}] is connected to the id = [{}] which is not valid",
                    server.id, id
                )));
            }
        }

        //a server should be connected to at least two drones
        let count = server.connected_drone_ids.len();
        if count < 2 {
            return Err(ConfigError::Validation(format!(
                "The server with id = [{}] should be connected to at least two drones but found: {}",
                server.id, count
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wg_2024::config::{Client, Drone, Server};

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


    #[test]
    fn test_non_unique_ids() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![Client { id: 1, connected_drone_ids: vec![] }],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The id = [1] is duplicated"));
        }
    }

    #[test]
    fn test_drone_duplicate_connection() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![2,2], pdr: 0.1 }],
            client: vec![Client { id: 2, connected_drone_ids: vec![] }],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The drone with id = [1] has a duplicate in the connected_node_ids list: [2]"), "got: {msg}");
        }
    }

    #[test]
    fn test_drone_self_connection() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![1], pdr: 0.1 }],
            client: vec![Client { id: 2, connected_drone_ids: vec![] }],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The drone with id = [1] cannot be connected to itself"), "got: {msg}");
        }
    }

    #[test]
    fn test_drone_unknown_connection() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![42], pdr: 0.1 }],
            client: vec![],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The drone with id = [1] is connected to an unknown node id = [42]"), "got: {msg}");
        }
        
    }
    
    #[test]
    fn test_client_connected_to_client() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![
                Client { id: 2, connected_drone_ids: vec![3] },
                Client { id: 3, connected_drone_ids: vec![] },
            ],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The client with id = [2] cannot be connected to another client (with id = [3])"), "got: {msg}");
        }
    }

    #[test]
    fn test_client_with_invalid_drone_count() {
        let config = Config {
            drone: vec![
                Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 },
                Drone { id: 2, connected_node_ids: vec![], pdr: 0.1 },
                Drone { id: 3, connected_node_ids: vec![], pdr: 0.1 },
            ],
            client: vec![Client { id: 4, connected_drone_ids: vec![1, 2, 3] }],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The client with id = [4] can be connected to at least one and at most two drones but found: 3"), "got: {msg}");
        }
    }

    #[test]
    fn test_client_connected_to_server() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![Client { id: 2, connected_drone_ids: vec![3] }],
            server: vec![Server { id: 3, connected_drone_ids: vec![1] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The client with id = [2] cannot be connected to a server (with id = [3])"), "got: {msg}");
        }
    }

    #[test]
    fn test_duplicate_client_connection(){
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![Client { id: 2, connected_drone_ids: vec![1, 1] }],
            server: vec![Server { id: 3, connected_drone_ids: vec![1] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The client with id = [2] has a duplicate in the drone's list, which is: id = [1]"), "got: {msg}");
        }
    }

    #[test]
    fn test_invalid_client_connection(){
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![Client { id: 2, connected_drone_ids: vec![1, 4] }],
            server: vec![Server { id: 3, connected_drone_ids: vec![1] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The client with id = [2] is connected to the id = [4] which is not valid"), "got: {msg}");
        }
    }
    
    #[test]
    fn test_server_connected_to_client() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![Client { id: 2, connected_drone_ids: vec![1] }],
            server: vec![Server { id: 3, connected_drone_ids: vec![2] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The server with id = [3] cannot be connected to a client (with id = [2])"), "got: {msg}");
        }
    }

    #[test]
    fn test_server_connected_to_server() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }, Drone { id: 2, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![Client { id: 3, connected_drone_ids: vec![1] }],
            server: vec![Server { id: 4, connected_drone_ids: vec![1, 2] }, Server { id: 5, connected_drone_ids: vec![4] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The server with id = [5] cannot be connected to another server (with id = [4])"), "got: {msg}");
        }
    }

    #[test]
    fn test_duplicate_server_connection() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }, Drone { id: 2, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![Client { id: 3, connected_drone_ids: vec![1] }],
            server: vec![Server { id: 4, connected_drone_ids: vec![1, 1] }, Server { id: 5, connected_drone_ids: vec![4] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The server with id = [4] has a duplicate in the drone's list, which is: id = [1]"), "got: {msg}");
        }
    }

    #[test]
    fn test_invalid_server_connection() {
        let config = Config {
            drone: vec![Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 }, Drone { id: 2, connected_node_ids: vec![], pdr: 0.1 }],
            client: vec![Client { id: 3, connected_drone_ids: vec![1] }],
            server: vec![Server { id: 4, connected_drone_ids: vec![1, 5] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The server with id = [4] is connected to the id = [5] which is not valid"), "got: {msg}");
        }
    }

    #[test]
    fn test_server_with_invalid_drone_count() {
        let config = Config {
            drone: vec![
                Drone { id: 1, connected_node_ids: vec![], pdr: 0.1 },
                Drone { id: 2, connected_node_ids: vec![], pdr: 0.1 },
            ],
            client: vec![Client { id: 3, connected_drone_ids: vec![1, 2] }],
            server: vec![Server { id: 4, connected_drone_ids: vec![1] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The server with id = [4] should be connected to at least two drones but found: 1"), "got: {msg}");
        }
    }

    #[test]
    fn test_non_bidirectional_graph_should_fail() {
        let config = Config {
            drone: vec![
                Drone { id: 1, connected_node_ids: vec![2], pdr: 0.1 },
                Drone { id: 2, connected_node_ids: vec![], pdr: 0.1 },
            ],
            client: vec![],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The graph is not bidirectional"), "got: {msg}");
        }
    }

    #[test]
    fn test_client_server_not_at_edge() {
        let config = Config {
            drone: vec![
                Drone { id: 1, connected_node_ids: vec![3, 4], pdr: 0.1 },
                Drone { id: 2, connected_node_ids: vec![3, 4], pdr: 0.1 },
            ],
            client: vec![Client { id: 3, connected_drone_ids: vec![1, 2] }],
            server: vec![Server { id: 4, connected_drone_ids: vec![1, 2] }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(
                msg.contains("Clients and/ or severs aren't at the edge of the network"));
        }
    }

    #[test]
    fn test_valid_config() {
        let config = Config {
            drone: vec![
                Drone { id: 1, connected_node_ids: vec![2, 3, 4], pdr: 0.1 },
                Drone { id: 2, connected_node_ids: vec![1, 4], pdr: 0.1 },
            ],
            client: vec![Client { id: 3, connected_drone_ids: vec![1] }],
            server: vec![Server { id: 4, connected_drone_ids: vec![1, 2] }],
        };

        let result = validate(&config);
        assert!(result.is_ok());
    }
}