#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use super::*;
    use crossbeam_channel::{unbounded, Receiver, Sender};
    use std::thread;
    use std::time::Duration;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{Fragment, Ack, Nack, FloodResponse, PacketType, Packet, NackType, FloodRequest, NodeType};
    use message::{NodeCommand, NodeEvent, SentMessageWrapper};
    use message::ChatResponse::ClientList;
    use crate::ChatServer;

    // Helper function per creare un ChatServer di test
    fn create_test_server() -> (ChatServer, Receiver<NodeEvent>, Sender<NodeCommand>, Sender<Packet>) {
        let (controller_send, controller_recv_test) = unbounded();
        let (controller_send_test, controller_recv) = unbounded();
        let (packet_send_test, packet_recv) = unbounded();

        let server = ChatServer::new(
            1, // server id
            controller_send,
            controller_recv,
            packet_recv,
            HashMap::new(),
        );

        (server, controller_recv_test, controller_send_test, packet_send_test)
    }

    // Helper function per creare un packet di test
    fn create_test_packet(session_id: u64, source: NodeId, destination: NodeId, pack_type: PacketType) -> Packet {
        Packet {
            routing_header: SourceRoutingHeader::initialize(vec![source, destination]),
            session_id,
            pack_type,
        }
    }

    #[test]
    fn test_add_sender_command() {
        let (mut server, event_recv, command_send, _) = create_test_server();
        let (new_sender, _) = unbounded();

        // Test aggiunta di un nuovo sender
        let command = NodeCommand::AddSender(2, new_sender);
        server.command_handler(command);

        assert!(server.packet_send.contains_key(&2));

        // Verifica che sia stato inviato un flood request
        // (dovrebbe essere gestito da flood_initializer)
    }

    #[test]
    fn test_remove_sender_command() {
        let (mut server, _, _, _) = create_test_server();
        let (sender, _) = unbounded();

        // Prima aggiungiamo un sender
        server.packet_send.insert(2, sender);

        // Poi lo rimuoviamo
        let command = NodeCommand::RemoveSender(2);
        server.command_handler(command);

        assert!(!server.packet_send.contains_key(&2));
    }

    #[test]
    fn test_msg_fragment_handling() {
        let (mut server, event_recv, _, _) = create_test_server();

        // Registra un client usando il metodo corretto
        server.server_message_manager.add_to_registered_client(2);

        // Crea un fragment di test
        let mut data = [0u8; 128];
        let hello_bytes = b"Hello";
        data[..hello_bytes.len()].copy_from_slice(hello_bytes);

        let fragment = Fragment {
            fragment_index: 0,
            total_n_fragments: 1,
            length: 5,
            data,
        };

        let packet = create_test_packet(
            1,
            2, // source
            1, // destination (server)
            PacketType::MsgFragment(fragment)
        );

        server.packet_handler(packet);

        // Verifica che il messaggio sia stato ricevuto
        let event = event_recv.try_recv().unwrap();
        match event {
            NodeEvent::MessageRecv(wrapper) => {
                // Il messaggio ora è un RecvMessageWrapper, non una stringa
                assert_eq!(wrapper.session_id, 1);
                assert_eq!(wrapper.source, 2);
            }
            _ => panic!("Expected MessageRecv event"),
        }
    }

    #[test]
    fn test_msg_fragment_unregistered_client() {
        let (mut server, event_recv, _, _) = create_test_server();

        // Non registriamo il client
        let mut data = [0u8; 128];
        let hello_bytes = b"Hello";
        data[..hello_bytes.len()].copy_from_slice(hello_bytes);

        let fragment = Fragment {
            fragment_index: 0,
            total_n_fragments: 1,
            length: 5,
            data,
        };

        let packet = create_test_packet(
            1,
            2, // source non registrata
            1, // destination
            PacketType::MsgFragment(fragment)
        );

        server.packet_handler(packet);

        // Non dovrebbe esserci nessun evento
        assert!(event_recv.try_recv().is_err());
    }

    #[test]
    fn test_ack_handling() {
        let (mut server, _, _, _) = create_test_server();

        // Prima crea un messaggio outgoing per il test
        let session_id = 1;
        let wrapper = SentMessageWrapper::from_message(session_id, 2, &ClientList(vec![0]));
        server.server_message_manager.outgoing_packets.insert(session_id, wrapper);

        // Aggiungi il nodo alla topologia del NetworkManager
        server.network_manager.topology.insert(2, (HashSet::new(), 1.0, 1.0));

        let ack = Ack {
            fragment_index: 0,
        };

        let packet = create_test_packet(
            session_id,
            2, // source
            1, // destination
            PacketType::Ack(ack)
        );

        server.packet_handler(packet);

        // Verifica che il network_manager sia stato aggiornato
        let node_stats = server.network_manager.topology.get(&2).unwrap();
        assert_eq!(node_stats.1, 2.0); // successi incrementati
        assert_eq!(node_stats.2, 2.0); // totale incrementato
    }

    #[test]
    fn test_nack_handling() {
        let (mut server, _, _, _) = create_test_server();

        // Prepara la topologia
        server.network_manager.topology.insert(2, (HashSet::new(), 1.0, 1.0));
        server.network_manager.topology.insert(3, (HashSet::new(), 1.0, 1.0));

        // Crea un messaggio outgoing per il test
        let session_id = 1;
        let wrapper = SentMessageWrapper::from_message(session_id, 3, &ClientList(vec![0]));
        server.server_message_manager.outgoing_packets.insert(session_id, wrapper);

        let nack = Nack {
            fragment_index: 0,
            nack_type: NackType::ErrorInRouting(3),
        };

        let mut packet = create_test_packet(
            session_id,
            2, // source
            1, // destination
            PacketType::Nack(nack)
        );
        packet.routing_header.hops = vec![2, 1]; // Imposta il primo hop

        server.packet_handler(packet);

        // Verifica che il nodo problematico sia stato rimosso
        assert!(!server.network_manager.topology.contains_key(&3));

        // Verifica che le statistiche del nodo source siano aggiornate
        let node_stats = server.network_manager.topology.get(&2).unwrap();
        assert_eq!(node_stats.2, 2.0); // totale incrementato
        assert_eq!(server.network_manager.n_errors, 1); // errori incrementati
    }

    #[test]
    fn test_flood_request_handling() {
        let (mut server, _, _, _) = create_test_server();

        let flood_request = FloodRequest::initialize(0, 2, NodeType::Drone);

        let packet = create_test_packet(
            1,
            2, // source
            1, // destination
            PacketType::FloodRequest(flood_request)
        );

        // Aggiungi un sender per poter inviare la risposta
        let (sender, receiver) = unbounded();
        server.packet_send.insert(2, sender);

        server.packet_handler(packet);

        // Verifica che sia stata inviata una FloodResponse
        let response_packet = receiver.try_recv().unwrap();
        match response_packet.pack_type {
            PacketType::FloodResponse(flood_response) => {
                assert_eq!(flood_response.flood_id, 0);
                // Verifica che il server sia stato aggiunto al path_trace
                assert!(flood_response.path_trace.iter().any(|(id, node_type)|
                    *id == 1 && *node_type == NodeType::Server
                ));
            }
            _ => panic!("Expected FloodResponse"),
        }
    }

    #[test]
    fn test_flood_response_handling() {
        let (mut server, _, _, _) = create_test_server();

        let flood_response = FloodResponse {
            flood_id: 0,
            path_trace: vec![
                (2, NodeType::Drone),
                (3, NodeType::Client),
                (1, NodeType::Server)
            ],
        };

        let packet = create_test_packet(
            1,
            2, // source
            1, // destination
            PacketType::FloodResponse(flood_response)
        );

        server.packet_handler(packet);

        // Verifica che la topologia sia stata aggiornata
        assert!(server.network_manager.topology.contains_key(&2));
        assert!(server.network_manager.topology.contains_key(&3));

        // Verifica che il client sia stato aggiunto alla lista client
        assert!(server.network_manager.client_list.contains(&3));

        // Verifica che ci sia una connessione tra i nodi
        let drone_connections = &server.network_manager.topology.get(&2).unwrap().0;
        assert!(drone_connections.contains(&3));
    }

    #[test]
    fn test_send_packet_success() {
        let (mut server, event_recv, _, _) = create_test_server();
        let (sender, receiver) = unbounded();

        // Aggiungi un sender per il nodo 2
        server.packet_send.insert(2, sender);

        let mut packet = create_test_packet(
            1,
            1, // source
            2, // destination
            PacketType::Ack(Ack { fragment_index: 0 })
        );

        server.send_packet(&mut packet);

        // Verifica che il pacchetto sia stato inviato
        let received_packet = receiver.try_recv().unwrap();
        assert_eq!(received_packet.session_id, 1);

        // Verifica che sia stato inviato un evento PacketSent
        let event = event_recv.try_recv().unwrap();
        match event {
            NodeEvent::PacketSent(sent_packet) => {
                assert_eq!(sent_packet.session_id, 1);
            }
            _ => panic!("Expected PacketSent event"),
        }
    }

    #[test]
    fn test_send_packet_unreachable_drone() {
        let (mut server, event_recv, _, _) = create_test_server();
        let (sender, _) = unbounded::<Packet>();

        // Chiudi il receiver per simulare un drone irraggiungibile
        drop(sender);
        server.packet_send.insert(2, crossbeam_channel::unbounded().0);

        let mut packet = create_test_packet(
            1,
            1, // source
            2, // destination
            PacketType::Ack(Ack { fragment_index: 0 })
        );

        server.send_packet(&mut packet);

        // Verifica che il pacchetto sia stato aggiunto al buffer
        assert!(server.server_buffer.contains_key(&1)); // source del pacchetto
    }

    #[test]
    fn test_try_resend_with_route_update() {
        let (mut server, event_recv, _, _) = create_test_server();
        let (sender, receiver) = unbounded();

        // Prepara la topologia e le route
        server.network_manager.topology.insert(2, (HashSet::new(), 1.0, 1.0));
        server.network_manager.topology.insert(3, (HashSet::new(), 1.0, 1.0));
        server.network_manager.client_list.insert(3);
        server.network_manager.routes.insert(3, vec![1, 2, 3]);

        // Aggiungi un sender
        server.packet_send.insert(2, sender);

        // Crea un pacchetto con routing header verso il client 3
        let mut packet = create_test_packet(
            1,
            1, // source
            3, // destination (client)
            PacketType::Ack(Ack { fragment_index: 0 })
        );
        packet.routing_header = SourceRoutingHeader::initialize(vec![1, 2, 3]);

        // Aggiungi al buffer
        server.add_to_buffer(packet);

        // Chiama try_resend
        server.try_resend();

        // Verifica che il buffer sia vuoto
        assert!(server.server_buffer.is_empty());

        // Verifica che il pacchetto sia stato inviato con routing aggiornato
        let sent_packet = receiver.try_recv().unwrap();
        assert_eq!(sent_packet.routing_header.hops, vec![1, 2, 3]);

        // Verifica l'evento PacketSent
        let event = event_recv.try_recv().unwrap();
        match event {
            NodeEvent::PacketSent(_) => {},
            _ => panic!("Expected PacketSent event"),
        }
    }

    #[test]
    fn test_flood_initializer() {
        let (mut server, _, _, _) = create_test_server();
        let (sender1, receiver1) = unbounded();
        let (sender2, receiver2) = unbounded();

        // Aggiungi alcuni sender
        server.packet_send.insert(2, sender1);
        server.packet_send.insert(3, sender2);

        let initial_session_id = server.last_session_id;

        server.flood_initializer();

        // Verifica che il session_id sia incrementato
        assert_eq!(server.last_session_id, initial_session_id + 1);

        // Verifica che i flood request siano stati inviati
        let packet1 = receiver1.try_recv().unwrap();
        let packet2 = receiver2.try_recv().unwrap();

        match packet1.pack_type {
            PacketType::FloodRequest(_) => {},
            _ => panic!("Expected FloodRequest"),
        }

        match packet2.pack_type {
            PacketType::FloodRequest(_) => {},
            _ => panic!("Expected FloodRequest"),
        }
    }

    #[test]
    fn test_from_shortcut_command() {
        let (mut server, event_recv, _, _) = create_test_server();

        // Prepara la topologia per l'ack
        server.network_manager.topology.insert(2, (HashSet::new(), 1.0, 1.0));

        let packet = create_test_packet(
            1,
            2, // source
            1, // destination
            PacketType::Ack(Ack { fragment_index: 0 })
        );

        let command = NodeCommand::FromShortcut(packet.clone());
        server.command_handler(command);

        // Verifica che il network_manager sia stato aggiornato (come per un ack normale)
        let node_stats = server.network_manager.topology.get(&2).unwrap();
        assert_eq!(node_stats.1, 2.0); // successi incrementati
        assert_eq!(node_stats.2, 2.0); // totale incrementato
    }

    #[test]
    fn test_add_to_buffer() {
        let (mut server, _, _, _) = create_test_server();

        let packet1 = create_test_packet(1, 2, 1, PacketType::Ack(Ack { fragment_index: 0 }));
        let packet2 = create_test_packet(2, 2, 1, PacketType::Ack(Ack { fragment_index: 1 }));

        server.add_to_buffer(packet1);
        server.add_to_buffer(packet2);

        // Verifica che i pacchetti siano nel buffer
        assert!(server.server_buffer.contains_key(&2));
        assert_eq!(server.server_buffer.get(&2).unwrap().len(), 2);
    }

    #[test]
    #[should_panic(expected = "Controller is unreaceable")]
    fn test_send_event_unreachable_controller() {
        let (controller_send, _) = unbounded::<NodeEvent>();
        let (_, controller_recv) = unbounded::<NodeCommand>();
        let (_, packet_recv) = unbounded::<Packet>();

        // Chiudi il receiver per simulare un controller irraggiungibile
        drop(controller_send);

        let server = ChatServer::new(
            1,
            crossbeam_channel::unbounded().0, // sender che fallirà
            controller_recv,
            packet_recv,
            HashMap::new(),
        );

        server.send_event(NodeEvent::PacketSent(create_test_packet(
            1, 1, 2, PacketType::Ack(Ack { fragment_index: 0 })
        )));
    }

    // Test di integrazione per simulare un flusso completo
    #[test]
    fn test_complete_message_flow() {
        let (mut server, event_recv, command_send, packet_send) = create_test_server();
        let (drone_sender, drone_recv) = unbounded();

        // Registra un client
        server.server_message_manager.add_to_registered_client(2);

        // Aggiungi un drone
        command_send.send(NodeCommand::AddSender(3, drone_sender.clone())).unwrap();
        server.command_handler(NodeCommand::AddSender(3, drone_sender));

        // Simula la ricezione di un fragment
        let mut data = [0u8; 128];
        let hello_bytes = b"Hello";
        data[..hello_bytes.len()].copy_from_slice(hello_bytes);

        let fragment = Fragment {
            fragment_index: 0,
            total_n_fragments: 1,
            length: 5,
            data,
        };

        let packet = create_test_packet(
            1,
            2,
            1,
            PacketType::MsgFragment(fragment)
        );

        packet_send.send(packet.clone()).unwrap();
        server.packet_handler(packet);

        // Verifica eventi
        let event = event_recv.try_recv().unwrap();
        match event {
            NodeEvent::MessageRecv(wrapper) => {
                assert_eq!(wrapper.session_id, 1);
                assert_eq!(wrapper.source, 2);
            }
            _ => panic!("Expected MessageRecv event"),
        }
    }

    // Test aggiuntivi per scenari specifici del NetworkManager
    #[test]
    fn test_should_flood_request_on_errors() {
        let (mut server, _, _, _) = create_test_server();

        // Simula errori multipli
        server.network_manager.n_errors = 7; // dovrebbe triggerare flood
        assert!(server.network_manager.should_flood_request());

        server.network_manager.n_errors = 14; // dovrebbe triggerare flood
        assert!(server.network_manager.should_flood_request());
    }

    #[test]
    fn test_should_flood_request_on_drops() {
        let (mut server, _, _, _) = create_test_server();

        // Simula drop multipli
        server.network_manager.n_dropped = 3; // dovrebbe triggerare flood
        assert!(server.network_manager.should_flood_request());

        server.network_manager.n_dropped = 6; // dovrebbe triggerare flood
        assert!(server.network_manager.should_flood_request());
    }

    #[test]
    fn test_nack_dropped_handling() {
        let (mut server, _, _, _) = create_test_server();

        // Prepara la topologia
        server.network_manager.topology.insert(2, (HashSet::new(), 1.0, 1.0));

        let nack = Nack {
            fragment_index: 0,
            nack_type: NackType::Dropped,
        };

        let mut packet = create_test_packet(
            1,
            2, // source
            1, // destination
            PacketType::Nack(nack)
        );
        packet.routing_header.hops = vec![2, 1];

        server.packet_handler(packet);

        // Verifica che i drop siano incrementati
        assert_eq!(server.network_manager.n_dropped, 1);
    }

    #[test]
    fn test_network_manager_route_calculation() {
        let (mut server, _, _, _) = create_test_server();

        // Prepara una topologia più complessa
        let mut drone_connections = HashSet::new();
        drone_connections.insert(3); // drone connesso al client
        server.network_manager.topology.insert(2, (drone_connections, 1.0, 1.0));
        server.network_manager.topology.insert(3, (HashSet::new(), 1.0, 1.0));
        server.network_manager.client_list.insert(3);

        // Aggiungi il server alla topologia con connessione al drone
        let mut server_connections = HashSet::new();
        server_connections.insert(2);
        server.network_manager.topology.insert(1, (server_connections, 1.0, 1.0));

        // Genera le route
        server.network_manager.generate_all_routes();

        // Verifica che la route al client sia calcolata correttamente
        let route = server.network_manager.get_route(&3);
        assert!(route.is_some());
        let route = route.unwrap();
        assert_eq!(route[0], 1); // parte dal server
        assert_eq!(route[route.len() - 1], 3); // arriva al client
    }
}