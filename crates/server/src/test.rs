#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
    use std::collections::{HashMap, HashSet};
    use std::thread;
    use std::time::Duration;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType};
    use message::{ChatRequest, ChatResponse, NodeCommand, NodeEvent};
    use crate::ChatServer;

    fn create_test_server() -> (ChatServer, Receiver<NodeEvent>, Sender<NodeCommand>, Sender<Packet>) {
        create_test_server_with_topology(vec![])
    }

    fn create_test_server_with_drone_topology(client_ids: Vec<NodeId>) -> (ChatServer, Receiver<NodeEvent>, Sender<NodeCommand>, Sender<Packet>) {
        let server_id = 1;
        let drone_id = 100; // ID del drone intermedio
        let (controller_send, controller_recv_events) = unbounded();
        let (controller_send_commands, controller_recv) = unbounded();
        let (packet_send_to_server, packet_recv) = unbounded();
        let packet_send = HashMap::new();

        let mut server = ChatServer::new(
            server_id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
        );

        // Inizializza la topologia del server
        // Il server ha se stesso nella topologia
        server.network_manager.topology.insert(server_id, (HashSet::new(), 1.0, 1.0));

        // Aggiungi il drone intermedio alla topologia
        server.network_manager.topology.insert(drone_id, (HashSet::new(), 1.0, 1.0));

        // Crea connessione server -> drone
        server.network_manager.topology.get_mut(&server_id).unwrap().0.insert(drone_id);

        // Aggiungi i client alla topologia e alle liste
        for client_id in client_ids.iter() {
            // Aggiungi il client alla topologia con probabilità di successo alta
            server.network_manager.topology.insert(*client_id, (HashSet::new(), 1.0, 1.0));
            server.network_manager.client_list.insert(*client_id);

            // Crea connessione drone -> client (non server -> client diretto)
            server.network_manager.topology.get_mut(&drone_id).unwrap().0.insert(*client_id);

            // Registra il client nel message manager
            server.server_message_manager.add_to_registered_client(*client_id);

            // Genera la route attraverso il drone: server -> drone -> client
            server.network_manager.routes.insert(*client_id, vec![server_id, drone_id, *client_id]);
        }

        (server, controller_recv_events, controller_send_commands, packet_send_to_server)
    }

    fn create_test_server_with_topology(client_ids: Vec<NodeId>) -> (ChatServer, Receiver<NodeEvent>, Sender<NodeCommand>, Sender<Packet>) {
        let server_id = 1;
        let (controller_send, controller_recv_events) = unbounded();
        let (controller_send_commands, controller_recv) = unbounded();
        let (packet_send_to_server, packet_recv) = unbounded();
        let packet_send = HashMap::new();

        let mut server = ChatServer::new(
            server_id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
        );

        // Inizializza la topologia del server
        // Il server ha se stesso nella topologia
        server.network_manager.topology.insert(server_id, (HashSet::new(), 1.0, 1.0));

        // Aggiungi i client alla topologia e alle liste
        for client_id in client_ids.iter() {
            // Aggiungi il client alla topologia con probabilità di successo alta
            server.network_manager.topology.insert(*client_id, (HashSet::new(), 1.0, 1.0));
            server.network_manager.client_list.insert(*client_id);

            // Crea una connessione diretta server -> client
            server.network_manager.topology.get_mut(&server_id).unwrap().0.insert(*client_id);

            // Registra il client nel message manager
            server.server_message_manager.add_to_registered_client(*client_id);

            // Genera la route diretta
            server.network_manager.routes.insert(*client_id, vec![server_id, *client_id]);
        }

        (server, controller_recv_events, controller_send_commands, packet_send_to_server)
    }

    fn create_fragment(index: u64, total: u64, data: &str) -> Fragment {
        let mut fragment_data = [0u8; 128];
        let bytes = data.as_bytes();
        let len = bytes.len().min(128);
        fragment_data[..len].copy_from_slice(&bytes[..len]);

        Fragment {
            fragment_index: index,
            total_n_fragments: total,
            length: len as u8,
            data: fragment_data,
        }
    }

    #[test]
    fn test_server_creation() {
        let (server, _, _, _) = create_test_server();
        assert_eq!(server.id, 1);
        assert_eq!(server.last_session_id, 0);
        assert!(server.server_buffer.is_empty());
        // Verifica che il server abbia se stesso nella topologia
        assert!(server.network_manager.topology.contains_key(&1));
    }

    #[test]
    fn test_add_sender_command() {
        let (mut server, events_recv, commands_send, _) = create_test_server();
        let (drone_send, _) = unbounded();
        let drone_id = 10;

        // Aggiungi un sender per un drone
        server.command_handler(NodeCommand::AddSender(drone_id, drone_send));

        assert!(server.packet_send.contains_key(&drone_id));
    }

    #[test]
    fn test_remove_sender_command() {
        let (mut server, _, _, _) = create_test_server();
        let (drone_send, _) = unbounded();
        let drone_id = 10;

        // Prima aggiungi, poi rimuovi
        server.command_handler(NodeCommand::AddSender(drone_id, drone_send));
        assert!(server.packet_send.contains_key(&drone_id));

        server.command_handler(NodeCommand::RemoveSender(drone_id));
        assert!(!server.packet_send.contains_key(&drone_id));
        // Verifica che il drone sia stato rimosso anche dalla topologia
        assert!(!server.network_manager.topology.contains_key(&drone_id));
    }

    #[test]
    fn test_fragment_handling_unregistered_client() {
        let (mut server, events_recv, _, _) = create_test_server();
        let client_id = 5;
        let session_id = 100;

        // Crea un frammento da un client non registrato
        let fragment = create_fragment(0, 1, "test message");
        let routing_header = SourceRoutingHeader::new(vec![client_id, server.id], 1);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::MsgFragment(fragment),
        };

        // Il server dovrebbe ignorare il messaggio da un client non registrato
        server.packet_handler(packet);

        // Non dovrebbe esserci nessun evento generato
        assert!(events_recv.try_recv().is_err());
    }

    #[test]
    fn test_client_registration() {
        let client_id = 5;
        let (mut server, events_recv, _, _) = create_test_server_with_topology(vec![client_id]);
        let session_id = 100;

        // Crea messaggio di registrazione
        let register_msg = ChatRequest::Register(client_id);
        let msg_str = serde_json::to_string(&register_msg).unwrap();
        let fragment = create_fragment(0, 1, &msg_str);

        let routing_header = SourceRoutingHeader::new(vec![client_id, server.id], 1);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::MsgFragment(fragment),
        };

        server.packet_handler(packet);

        // Verifica che il client sia registrato
        assert!(server.server_message_manager.is_registered(&client_id));

        // Verifica che sia stato generato un evento MessageRecv
        let event = events_recv.try_recv();
        assert!(event.is_ok());
    }

    #[test]
    fn test_message_fragmentation_and_reconstruction() {
        let client_id = 5;
        let (mut server, events_recv, _, _) = create_test_server_with_topology(vec![client_id]);
        let session_id = 100;

        // Crea un messaggio più lungo che richiede effettivamente più frammenti
        let send_msg = ChatRequest::SendMessage {
            from: client_id,
            to: 6,
            message: "A".repeat(150), // Messaggio lungo che richiede più di un frammento
        };
        let msg_str = serde_json::to_string(&send_msg).unwrap();
        let msg_bytes = msg_str.as_bytes();

        // Calcola il numero di frammenti necessari
        let fragment_size = 128;
        let total_fragments = (msg_bytes.len() + fragment_size - 1) / fragment_size;

        // Simula la frammentazione manuale
        let mut fragments = Vec::new();
        for i in 0..total_fragments {
            let start = i * fragment_size;
            let end = (start + fragment_size).min(msg_bytes.len());
            let chunk = &msg_bytes[start..end];

            let mut fragment_data = [0u8; 128];
            fragment_data[..chunk.len()].copy_from_slice(chunk);

            let fragment = Fragment {
                fragment_index: i as u64,
                total_n_fragments: total_fragments as u64,
                length: chunk.len() as u8,
                data: fragment_data,
            };
            fragments.push(fragment);
        }

        // Invia tutti i frammenti
        for fragment in fragments {
            let routing_header = SourceRoutingHeader::new(vec![client_id, server.id], 1);
            let packet = Packet {
                routing_header,
                session_id,
                pack_type: PacketType::MsgFragment(fragment),
            };
            server.packet_handler(packet);
        }

        // Verifica che sia stato generato un evento di messaggio ricevuto
        let event = events_recv.try_recv();
        assert!(event.is_ok());

        if let Ok(NodeEvent::MessageRecv(_)) = event {
            // Test passato
        } else {
            panic!("Expected MessageRecv event");
        }
    }

    #[test]
    fn test_ack_handling() {
        let client_id = 5;
        let (mut server, _, _, _) = create_test_server_with_topology(vec![client_id]);
        let session_id = 100;

        // Crea un Ack
        let ack = Ack { fragment_index: 0 };
        let routing_header = SourceRoutingHeader::new(vec![client_id, server.id], 1);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::Ack(ack),
        };

        // Prima crea un messaggio in uscita per poter ricevere l'ACK
        let test_msg = ChatResponse::ClientList(vec![1, 2, 3]);
        let wrapper = message::SentMessageWrapper::from_message(session_id, client_id, &test_msg);
        server.server_message_manager.outgoing_packets.insert(session_id, wrapper);

        server.packet_handler(packet);

        // Verifica che l'ACK sia stato processato
        // Il network manager dovrebbe essere stato aggiornato
        assert!(server.network_manager.topology.get(&client_id).unwrap().1 > 1.0);
    }

    #[test]
    fn test_nack_handling() {
        let drone_id = 10;
        let client_id = 5;
        let (mut server, _, _, _) = create_test_server_with_topology(vec![client_id]);
        let session_id = 100;

        // Aggiungi il drone alla topologia del network manager
        server.network_manager.topology.insert(drone_id, (std::collections::HashSet::new(), 1.0, 1.0));

        // Crea un messaggio in uscita
        let test_msg = ChatResponse::ClientList(vec![1, 2, 3]);
        let wrapper = message::SentMessageWrapper::from_message(session_id, client_id, &test_msg);
        server.server_message_manager.outgoing_packets.insert(session_id, wrapper);

        // Crea un NACK
        let nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };
        let routing_header = SourceRoutingHeader::new(vec![drone_id, server.id], 0);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::Nack(nack),
        };

        server.packet_handler(packet);

        // Verifica che il network manager abbia aggiornato le statistiche
        assert!(server.network_manager.n_dropped > 0);
    }

    #[test]
    fn test_flood_request_handling() {
        let (mut server, rx_nodeEvent, _, _) = create_test_server();
        let drone_id = 10;
        let session_id = 100;

        // Aggiungi un sender per poter inviare la risposta
        let (drone_send, drone_recv) = unbounded();
        server.packet_send.insert(drone_id, drone_send);

        // Crea una FloodRequest
        let flood_request = FloodRequest::initialize(0, drone_id, NodeType::Drone);
        let routing_header = SourceRoutingHeader::new(vec![drone_id, server.id], 1);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::FloodRequest(flood_request),
        };

        server.packet_handler(packet);
        let event = rx_nodeEvent.try_recv();

        // Verifica che sia stata inviata una FloodResponse
        let response = drone_recv.try_recv();
        assert!(response.is_ok());

        if let Ok(response_packet) = response {
            match response_packet.pack_type {
                PacketType::FloodResponse(_) => {
                    // Test passato
                }
                _ => panic!("Expected FloodResponse"),
            }
        }
    }

    #[test]
    fn test_flood_response_handling() {
        let (mut server, _, _, _) = create_test_server();

        // Crea una FloodResponse con path trace
        let flood_response = FloodResponse{
            flood_id: 0,
            path_trace: vec![
                (server.id, NodeType::Server),
                (10, NodeType::Drone),
                (5, NodeType::Client),]
        };

        let routing_header = SourceRoutingHeader::new(vec![5, 10, server.id], 1);
        let packet = Packet {
            routing_header,
            session_id: 100,
            pack_type: PacketType::FloodResponse(flood_response),
        };

        server.packet_handler(packet);

        // Verifica che la topologia sia stata aggiornata
        assert!(server.network_manager.topology.contains_key(&10));
        assert!(server.network_manager.client_list.contains(&5));
        // Verifica che le route siano state generate
        assert!(server.network_manager.routes.contains_key(&5));
    }

    #[test]
    fn test_send_message_request() {
        let sender_id = 5;
        let receiver_id = 6;
        let (mut server, events_recv, _, _) = create_test_server_with_topology(vec![sender_id, receiver_id]);
        let session_id = 100;

        // Crea messaggio di invio
        let send_msg = ChatRequest::SendMessage {
            from: sender_id,
            to: receiver_id,
            message: "Hello World".to_string(),
        };
        let msg_str = serde_json::to_string(&send_msg).unwrap();
        let fragment = create_fragment(0, 1, &msg_str);

        let routing_header = SourceRoutingHeader::new(vec![sender_id, server.id], 1);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::MsgFragment(fragment),
        };

        server.packet_handler(packet);

        // Verifica che sia stato generato un evento di messaggio ricevuto
        let event = events_recv.try_recv();
        assert!(event.is_ok());

        // Dovrebbe anche generare un evento CreateMessage per l'inoltro
        let create_event = events_recv.try_recv();
        assert!(create_event.is_ok());

        if let Ok(NodeEvent::CreateMessage(wrapper)) = create_event {
            assert_eq!(wrapper.destination, receiver_id);
        }
    }

    #[test]
    fn test_buffer_functionality() {
        let (mut server, _, _, _) = create_test_server();
        let dest_id = 5;

        // Crea un pacchetto per il buffer
        let routing_header = SourceRoutingHeader::new(vec![server.id, dest_id], 0);
        let packet = Packet {
            routing_header,
            session_id: 100,
            pack_type: PacketType::Ack(Ack { fragment_index: 0 }),
        };

        // Aggiungi al buffer
        server.add_to_buffer(packet.clone());

        assert!(server.server_buffer.contains_key(&dest_id));
        assert_eq!(server.server_buffer.get(&dest_id).unwrap().len(), 1);
    }

    #[test]
    fn test_try_resend() {
        let dest_id = 5;
        let (mut server, events_recv, _, _) = create_test_server_with_drone_topology(vec![dest_id]);

        // Aggiungi un sender per la destinazione
        let (dest_send, dest_recv) = unbounded();
        server.packet_send.insert(dest_id, dest_send);

        // Crea un pacchetto per il buffer
        let routing_header = SourceRoutingHeader::new(vec![server.id, dest_id], 0);
        let packet = Packet {
            routing_header,
            session_id: 100,
            pack_type: PacketType::Ack(Ack { fragment_index: 0 }),
        };

        // Aggiungi al buffer
        server.add_to_buffer(packet.clone());

        // Prova a reinviare
        server.try_resend();

        // Verifica che il buffer sia ora vuoto
        assert!(server.server_buffer.is_empty());

        // Verifica che il pacchetto sia stato inviato
        let sent_packet = dest_recv.try_recv();
        assert!(sent_packet.is_ok());
    }

    #[test]
    fn test_flood_initializer() {
        let (mut server, _, _, _) = create_test_server();

        // Aggiungi alcuni sender
        let (drone1_send, drone1_recv) = unbounded();
        let (drone2_send, drone2_recv) = unbounded();
        server.packet_send.insert(10, drone1_send);
        server.packet_send.insert(11, drone2_send);

        let initial_session_id = server.last_session_id;
        server.flood_initializer();

        // Verifica che il session_id sia incrementato
        assert_eq!(server.last_session_id, initial_session_id + 1);

        // Verifica che le FloodRequest siano state inviate a tutti i drone
        let flood1 = drone1_recv.try_recv();
        let flood2 = drone2_recv.try_recv();

        assert!(flood1.is_ok());
        assert!(flood2.is_ok());

        if let Ok(packet1) = flood1 {
            match packet1.pack_type {
                PacketType::FloodRequest(_) => {
                    // Test passato
                }
                _ => panic!("Expected FloodRequest"),
            }
        }
    }

    #[test]
    fn test_error_wrong_client_id() {
        let sender_id = 5;
        let unknown_receiver_id = 45;
        let (mut server, events_recv, _, _) = create_test_server_with_topology(vec![sender_id]);
        let session_id = 100;

        // Crea messaggio per client non registrato
        let send_msg = ChatRequest::SendMessage {
            from: sender_id,
            to: unknown_receiver_id,
            message: "Hello Unknown".to_string(),
        };
        let msg_str = serde_json::to_string(&send_msg).unwrap();
        let fragment = create_fragment(0, 1, &msg_str);

        let routing_header = SourceRoutingHeader::new(vec![sender_id, server.id], 1);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::MsgFragment(fragment),
        };

        server.packet_handler(packet);

        // Dovrebbe generare un evento di messaggio ricevuto e poi un CreateMessage con errore
        let _recv_event = events_recv.try_recv();
        let create_event = events_recv.try_recv();

        assert!(create_event.is_ok());
        if let Ok(NodeEvent::CreateMessage(wrapper)) = create_event {
            // Il messaggio dovrebbe contenere un errore e essere diretto al sender
            assert_eq!(wrapper.destination, sender_id);
        }
    }

    #[test]
    fn test_network_topology_initialization() {
        let client_ids = vec![5, 6, 7];
        let (server, _, _, _) = create_test_server_with_topology(client_ids.clone());

        // Verifica che tutti i client siano nella topologia
        for client_id in client_ids.iter() {
            assert!(server.network_manager.topology.contains_key(client_id));
            assert!(server.network_manager.client_list.contains(client_id));
            assert!(server.network_manager.routes.contains_key(client_id));
            assert!(server.server_message_manager.is_registered(client_id));
        }

        // Verifica che il server abbia se stesso nella topologia
        assert!(server.network_manager.topology.contains_key(&server.id));

        // Verifica che le route dirette siano state create correttamente
        for client_id in client_ids.iter() {
            let route = server.network_manager.routes.get(client_id).unwrap();
            assert_eq!(route, &vec![server.id, *client_id]);
        }
    }

    #[test]
    fn test_route_generation_after_topology_update() {
        let (mut server, _, _, _) = create_test_server();

        // Simula l'arrivo di una FloodResponse che aggiunge nuovi nodi
        let flood_response = FloodResponse{
            flood_id: 0,
            path_trace: vec![
                (server.id, NodeType::Server),
                (10, NodeType::Drone),
                (20, NodeType::Drone),
                (5, NodeType::Client),
            ]
        };

        let routing_header = SourceRoutingHeader::new(vec![server.id, 10, 20, 5], 1);
        let packet = Packet {
            routing_header,
            session_id: 100,
            pack_type: PacketType::FloodResponse(flood_response),
        };

        server.packet_handler(packet);

        // Verifica che la topologia sia stata aggiornata
        assert!(server.network_manager.topology.contains_key(&10));
        assert!(server.network_manager.topology.contains_key(&20));
        assert!(server.network_manager.client_list.contains(&5));

        // Verifica che le route siano state generate per il client
        assert!(server.network_manager.routes.contains_key(&5));

        // La route dovrebbe passare attraverso i drone
        let route = server.network_manager.routes.get(&5).unwrap();
        assert!(route.len() > 2); // Dovrebbe essere più di server -> client diretto
    }

    #[test]
    fn test_message_routing_through_drone() {
        let client_id = 5;
        let (mut server, events_recv, _, _) = create_test_server_with_drone_topology(vec![client_id]);
        let drone_id = 100;
        let session_id = 100;

        // Aggiungi il sender per il drone intermedio
        let (drone_send, drone_recv) = unbounded();
        server.packet_send.insert(drone_id, drone_send);

        // Crea messaggio di registrazione da inviare
        let register_msg = ChatRequest::Register(client_id);
        let msg_str = serde_json::to_string(&register_msg).unwrap();
        let fragment = create_fragment(0, 1, &msg_str);

        // Il routing header deve passare attraverso il drone
        let routing_header = SourceRoutingHeader::new(vec![client_id, drone_id, server.id], 2);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::MsgFragment(fragment),
        };

        server.packet_handler(packet);

        // Verifica che il client sia registrato
        assert!(server.server_message_manager.is_registered(&client_id));

        // Verifica che sia stato generato un evento MessageRecv
        let event = events_recv.try_recv();
        assert!(event.is_ok());
    }

    #[test]
    fn test_send_message_through_drone() {
        let sender_id = 5;
        let receiver_id = 6;
        let (mut server, events_recv, _, _) = create_test_server_with_drone_topology(vec![sender_id, receiver_id]);
        let drone_id = 100;
        let session_id = 100;

        // Aggiungi il sender per il drone intermedio
        let (drone_send, drone_recv) = unbounded();
        server.packet_send.insert(drone_id, drone_send);

        // Crea messaggio di invio
        let send_msg = ChatRequest::SendMessage {
            from: sender_id,
            to: receiver_id,
            message: "Hello through drone!".to_string(),
        };
        let msg_str = serde_json::to_string(&send_msg).unwrap();
        let fragment = create_fragment(0, 1, &msg_str);

        // Il routing header passa attraverso il drone
        let routing_header = SourceRoutingHeader::new(vec![sender_id, drone_id, server.id], 2);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::MsgFragment(fragment),
        };

        server.packet_handler(packet);

        // Verifica che sia stato generato un evento di messaggio ricevuto
        let event = events_recv.try_recv();
        assert!(event.is_ok());

        // Dovrebbe anche generare un evento CreateMessage per l'inoltro
        let create_event = events_recv.try_recv();
        assert!(create_event.is_ok());

        if let Ok(NodeEvent::CreateMessage(wrapper)) = create_event {
            assert_eq!(wrapper.destination, receiver_id);
        }

        // Verifica che quando il server invia la risposta, utilizzi il percorso attraverso il drone
        let sent_packet = drone_recv.try_recv();
        assert!(sent_packet.is_ok());

        if let Ok(packet) = sent_packet {
            // Il pacchetto dovrebbe essere diretto verso il drone
            assert_eq!(packet.routing_header.current_hop(), Some(drone_id));
            // E la destinazione finale dovrebbe essere il receiver
            assert_eq!(packet.routing_header.destination(), Some(receiver_id));
        }
    }

    #[test]
    fn test_multi_hop_topology() {
        let client_ids = vec![5, 6, 7];
        let (server, _, _, _) = create_test_server_with_drone_topology(client_ids.clone());
        let drone_id = 100;

        // Verifica che tutti i client siano nella topologia
        for client_id in client_ids.iter() {
            assert!(server.network_manager.topology.contains_key(client_id));
            assert!(server.network_manager.client_list.contains(client_id));
            assert!(server.network_manager.routes.contains_key(client_id));
            assert!(server.server_message_manager.is_registered(client_id));
        }

        // Verifica che il drone sia nella topologia
        assert!(server.network_manager.topology.contains_key(&drone_id));

        // Verifica che le route passino attraverso il drone
        for client_id in client_ids.iter() {
            let route = server.network_manager.routes.get(client_id).unwrap();
            assert_eq!(route, &vec![server.id, drone_id, *client_id]);
            assert!(route.len() == 3); // server -> drone -> client
            assert!(route.contains(&drone_id)); // Deve contenere il drone intermedio
        }

        // Verifica le connessioni nella topologia
        // Server deve essere connesso al drone
        assert!(server.network_manager.topology.get(&server.id).unwrap().0.contains(&drone_id));

        // Drone deve essere connesso ai client
        for client_id in client_ids.iter() {
            assert!(server.network_manager.topology.get(&drone_id).unwrap().0.contains(client_id));
        }
    }

    #[test]
    fn test_ack_through_drone() {
        let client_id = 5;
        let (mut server, _, _, _) = create_test_server_with_drone_topology(vec![client_id]);
        let drone_id = 100;
        let session_id = 100;

        // Crea un Ack che arriva attraverso il drone
        let ack = Ack { fragment_index: 0 };
        let routing_header = SourceRoutingHeader::new(vec![client_id, drone_id, server.id], 2);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::Ack(ack),
        };

        // Prima crea un messaggio in uscita per poter ricevere l'ACK
        let test_msg = ChatResponse::ClientList(vec![1, 2, 3]);
        let wrapper = message::SentMessageWrapper::from_message(session_id, client_id, &test_msg);
        server.server_message_manager.outgoing_packets.insert(session_id, wrapper);

        server.packet_handler(packet);

        // Verifica che l'ACK sia stato processato
        // Il network manager dovrebbe essere stato aggiornato per tutti i nodi nel percorso
        assert!(server.network_manager.topology.get(&client_id).unwrap().1 > 1.0);
        assert!(server.network_manager.topology.get(&drone_id).unwrap().1 > 1.0);
    }

    #[test]
    fn test_nack_through_drone() {
        let client_id = 5;
        let (mut server, _, _, _) = create_test_server_with_drone_topology(vec![client_id]);
        let drone_id = 100;
        let session_id = 100;

        // Crea un messaggio in uscita
        let test_msg = ChatResponse::ClientList(vec![1, 2, 3]);
        let wrapper = message::SentMessageWrapper::from_message(session_id, client_id, &test_msg);
        server.server_message_manager.outgoing_packets.insert(session_id, wrapper);

        // Crea un NACK dal drone
        let nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };
        let routing_header = SourceRoutingHeader::new(vec![drone_id, server.id], 0);
        let packet = Packet {
            routing_header,
            session_id,
            pack_type: PacketType::Nack(nack),
        };

        server.packet_handler(packet);

        // Verifica che il network manager abbia aggiornato le statistiche
        assert!(server.network_manager.n_dropped > 0);
        // Verifica che le statistiche del drone siano state aggiornate
        assert!(server.network_manager.topology.get(&drone_id).unwrap().2 > 1.0);
    }
}