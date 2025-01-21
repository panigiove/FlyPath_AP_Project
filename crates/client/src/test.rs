#[cfg(test)]
mod tests {
    use wg_2024::controller::{DroneCommand, DroneEvent};
    use wg_2024::drone::Drone;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{FloodRequest, NodeType, Packet};

    use std::collections::HashMap;
    use std::time::Duration;
    use crossbeam_channel::unbounded;
    use crossbeam_channel::{Receiver, Sender};
    use std::thread::{self, sleep};

    // Importing all drone implementations
    use lockheedrustin_drone::LockheedRustin;
    use rustbusters_drone::RustBustersDrone;
    use rustastic_drone::RustasticDrone;
    use ap2024_rustinpeace_nosounddrone::NoSoundDroneRIP;
    use bagel_bomber::BagelBomber;
    use LeDron_James::Drone as LeDronJames_drone;
    use rolling_drone::RollingDrone;
    use rust_roveri::RustRoveri;
    use rust_do_it::RustDoIt;

    use flyPath::FlyPath;

    // A-B-C-D-E-F-G-H-I-L
    fn build_complete_linear_network() -> (Vec<Sender<DroneCommand>>, Receiver<DroneEvent>, Receiver<Packet>, Receiver<Packet>, Vec<Sender<Packet>>, Vec<Box<dyn Drone>>){
        let mut send_drones_command: Vec<Sender<DroneCommand>> = Vec::new();
        let mut recv_drones_command: Vec<Receiver<DroneCommand>> = Vec::new();

        let (send_drone_event, recv_drone_event) = unbounded();

        let mut sends_packet: Vec<Sender<Packet>> = Vec::new();
        let mut recvs_packet: Vec<Receiver<Packet>> = Vec::new();
        
        for _ in 0..10{
            let (send_drone_command, recv_drone_command) = unbounded();
            send_drones_command.push(send_drone_command);
            recv_drones_command.push(recv_drone_command);

            let (send_packet, recv_packet) = unbounded();
            sends_packet.push(send_packet);
            recvs_packet.push(recv_packet);
        }

        let (client_send, client_recv) = unbounded();
        let (server_send, server_recv) = unbounded();

        let mut drones: Vec<Box<dyn Drone>> = Vec::new();

        let mut packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        packet_send_map.insert(10, client_send.clone());
        packet_send_map.insert(1, sends_packet[1].clone());
        drones.push(Box::new(LockheedRustin::new(0, send_drone_event.clone(), recv_drones_command[0].clone(), recvs_packet[0].clone(), packet_send_map.clone(), 0.0)));
        // drones.push(Box::new(FlyPath::new(0, send_drone_event.clone(), recv_drones_command[0].clone(), recvs_packet[0].clone(), packet_send_map.clone(), 0.0)));
        
        packet_send_map.clear();
        packet_send_map.insert(0, sends_packet[0].clone());
        packet_send_map.insert(2, sends_packet[2].clone());
        drones.push(Box::new(RustBustersDrone::new(1, send_drone_event.clone(), recv_drones_command[1].clone(), recvs_packet[1].clone(), packet_send_map.clone(), 0.0)));
        // drones.push(Box::new(FlyPath::new(1, send_drone_event.clone(), recv_drones_command[1].clone(), recvs_packet[1].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(1, sends_packet[1].clone());
        packet_send_map.insert(3, sends_packet[3].clone());
        drones.push(Box::new(RustasticDrone::new(2, send_drone_event.clone(), recv_drones_command[2].clone(), recvs_packet[2].clone(), packet_send_map.clone(), 0.0)));
        // drones.push(Box::new(FlyPath::new(2, send_drone_event.clone(), recv_drones_command[2].clone(), recvs_packet[2].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(2, sends_packet[2].clone());
        packet_send_map.insert(4, sends_packet[4].clone());
        drones.push(Box::new(NoSoundDroneRIP::new(3, send_drone_event.clone(), recv_drones_command[3].clone(), recvs_packet[3].clone(), packet_send_map.clone(), 0.0))); // not good enough
        // drones.push(Box::new(FlyPath::new(3, send_drone_event.clone(), recv_drones_command[3].clone(), recvs_packet[3].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(3, sends_packet[3].clone());
        packet_send_map.insert(5, sends_packet[5].clone());
        // drones.push(Box::new(BagelBomber::new(4, send_drone_event.clone(), recv_drones_command[4].clone(), recvs_packet[4].clone(), packet_send_map.clone(), 0.0))); // strange behavior
        drones.push(Box::new(FlyPath::new(4, send_drone_event.clone(), recv_drones_command[4].clone(), recvs_packet[4].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(4, sends_packet[4].clone());
        packet_send_map.insert(6, sends_packet[6].clone());
        drones.push(Box::new(LeDronJames_drone::new(5, send_drone_event.clone(), recv_drones_command[5].clone(), recvs_packet[5].clone(), packet_send_map.clone(), 0.0)));
        // drones.push(Box::new(FlyPath::new(5, send_drone_event.clone(), recv_drones_command[5].clone(), recvs_packet[5].clone(), packet_send_map.clone(), 0.0)));
        
        packet_send_map.clear();
        packet_send_map.insert(5, sends_packet[5].clone());
        packet_send_map.insert(7, sends_packet[7].clone());
        drones.push(Box::new(RollingDrone::new(6, send_drone_event.clone(), recv_drones_command[6].clone(), recvs_packet[6].clone(), packet_send_map.clone(), 0.0)));
        // drones.push(Box::new(FlyPath::new(6, send_drone_event.clone(), recv_drones_command[6].clone(), recvs_packet[6].clone(), packet_send_map.clone(), 0.0)));
        
        packet_send_map.clear();
        packet_send_map.insert(6, sends_packet[6].clone());
        packet_send_map.insert(8, sends_packet[8].clone());
        drones.push(Box::new(RustBustersDrone::new(7, send_drone_event.clone(), recv_drones_command[7].clone(), recvs_packet[7].clone(), packet_send_map.clone(), 0.0)));
        // drones.push(Box::new(FlyPath::new(7, send_drone_event.clone(), recv_drones_command[7].clone(), recvs_packet[7].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(7, sends_packet[7].clone());
        packet_send_map.insert(9, sends_packet[9].clone());
        drones.push(Box::new(RustRoveri::new(8, send_drone_event.clone(), recv_drones_command[8].clone(), recvs_packet[8].clone(), packet_send_map.clone(), 0.0)));
        // drones.push(Box::new(FlyPath::new(8, send_drone_event.clone(), recv_drones_command[8].clone(), recvs_packet[8].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(8, sends_packet[8].clone());
        packet_send_map.insert(11, server_send.clone());
        drones.push(Box::new(RustDoIt::new(9, send_drone_event.clone(), recv_drones_command[9].clone(), recvs_packet[9].clone(), packet_send_map.clone(), 0.0)));
        // drones.push(Box::new(FlyPath::new(9, send_drone_event.clone(), recv_drones_command[9].clone(), recvs_packet[9].clone(), packet_send_map.clone(), 0.0)));

        (send_drones_command, recv_drone_event, client_recv, server_recv, sends_packet, drones)
    }

    #[test]
    fn test_flood_request_and_flood_response() {
        let (_send_command, recv_event, recv_client, recv_server, send_drones, drones) = build_complete_linear_network();
        let mut handlers = Vec::new();

        // run drones
        for (i, mut drone) in drones.into_iter().enumerate() {
            let thread_name = format!("drone_thread_{}", i);
            handlers.push(thread::Builder::new()
                .name(thread_name)  // Assegna il nome al thread
                .spawn(move || {
                    drone.run();  // Esegui il metodo run sul drone
                })
                .unwrap());
        }
        
        // generate the flood request
        let flood_request = FloodRequest::initialize(0, 10, NodeType::Client);
        let init_flood = Packet::new_flood_request(SourceRoutingHeader::empty_route(), 0, flood_request.clone());

        // send the flood request to the first drone and sleep
        send_drones[0].send(init_flood.clone()).unwrap();
        sleep(Duration::from_secs(1));

        // server receive the flood request
        let received = recv_server.recv_timeout(Duration::from_secs(1));
        
        // create the expected flood request
        let flood_request_expected: FloodRequest = flood_request
            .get_incremented(0, NodeType::Drone)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(4, NodeType::Drone)
            .get_incremented(5, NodeType::Drone)
            .get_incremented(6, NodeType::Drone)
            .get_incremented(7, NodeType::Drone)
            .get_incremented(8, NodeType::Drone)
            .get_incremented(9, NodeType::Drone);
        let init_flood_expected = Packet::new_flood_request(SourceRoutingHeader::empty_route(), 0, flood_request_expected.clone());
        
        assert!(received.is_ok()); // does the server receive something?
        if let Ok(packet) = received {
            assert_eq!(packet.pack_type, init_flood_expected.pack_type); // check if we received what we expect

            // read or empty the controller drone event
            while let Ok(event) = recv_event.recv_timeout(Duration::from_secs(1)) {
                // println!("Controller: {:?}", event);
            }

            // generate the flood response
            let mut flood_response_expected = FloodRequest::initialize(0, 10, NodeType::Client)
                .get_incremented(0, NodeType::Drone)
                .get_incremented(1, NodeType::Drone)
                .get_incremented(2, NodeType::Drone)
                .get_incremented(3, NodeType::Drone)
                .get_incremented(4, NodeType::Drone)
                .get_incremented(5, NodeType::Drone)
                .get_incremented(6, NodeType::Drone)
                .get_incremented(7, NodeType::Drone)
                .get_incremented(8, NodeType::Drone)
                .get_incremented(9, NodeType::Drone)
                .get_incremented(11, NodeType::Server)
                .generate_response(0);
            flood_response_expected.routing_header.increase_hop_index();

            // send the flood response
            send_drones[9].send(flood_response_expected.clone()).unwrap();
            sleep(Duration::from_secs(1));

            // client receive the flood response
            let received = recv_client.recv_timeout(Duration::from_secs(1));

            assert!(received.is_ok()); // does the client receive something?
            if let Ok(packet) = received{
                // read or empty the controller drone event
                while let Ok(event) = recv_event.recv_timeout(Duration::from_secs(1)) {
                    println!("Controller: {:?}", event);
                }
                
                assert_eq!(packet.pack_type, flood_response_expected.pack_type); // check if we received what we expect
            }
        }
    }

    #[test]
    fn test_flood_request() {
        let (_send_command, recv_event, _recv_client, recv_server, send_drones, drones) = build_complete_linear_network();
        let mut handlers = Vec::new();
        for (i, mut drone) in drones.into_iter().enumerate() {
            let thread_name = format!("drone_thread_{}", i);
            handlers.push(thread::Builder::new()
                .name(thread_name)  // Assegna il nome al thread
                .spawn(move || {
                    drone.run();  // Esegui il metodo run sul drone
                })
                .unwrap());
        }
        
        let flood_request = FloodRequest::initialize(0, 10, NodeType::Client);
        let init_flood = Packet::new_flood_request(SourceRoutingHeader::empty_route(), 0, flood_request.clone());

        send_drones[0].send(init_flood.clone()).unwrap();
        sleep(Duration::from_secs(1));
        let received = recv_server.recv_timeout(Duration::from_secs(1));
        
        let flood_request_expected: FloodRequest = flood_request
            .get_incremented(0, NodeType::Drone)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(4, NodeType::Drone)
            .get_incremented(5, NodeType::Drone)
            .get_incremented(6, NodeType::Drone)
            .get_incremented(7, NodeType::Drone)
            .get_incremented(8, NodeType::Drone)
            .get_incremented(9, NodeType::Drone);
        let init_flood_expected = Packet::new_flood_request(SourceRoutingHeader::empty_route(), 0, flood_request_expected.clone());
        
        assert!(received.is_ok());
        if let Ok(packet) = received {
            assert_eq!(packet.pack_type, init_flood_expected.pack_type);

            while let Ok(event) = recv_event.recv_timeout(Duration::from_secs(1)) {
                // println!("Controller: {:?}", event);
            }
        }
    }

    #[test]
    fn test_flood_response(){
        let (_send_command, recv_event, recv_client, _recv_server, send_drones, drones) = build_complete_linear_network();
        let mut handlers = Vec::new();
        for (i, mut drone) in drones.into_iter().enumerate() {
            let thread_name = format!("drone_thread_{}", i);
            handlers.push(thread::Builder::new()
                .name(thread_name)  // Assegna il nome al thread
                .spawn(move || {
                    drone.run();  // Esegui il metodo run sul drone
                })
                .unwrap());
        }

        let mut flood_response_expected = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(0, NodeType::Drone)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(4, NodeType::Drone)
            .get_incremented(5, NodeType::Drone)
            .get_incremented(6, NodeType::Drone)
            .get_incremented(7, NodeType::Drone)
            .get_incremented(8, NodeType::Drone)
            .get_incremented(9, NodeType::Drone)
            .get_incremented(11, NodeType::Server)
            .generate_response(0);
        flood_response_expected.routing_header.increase_hop_index();

        send_drones[9].send(flood_response_expected.clone()).unwrap();
        sleep(Duration::from_secs(1));
        let received = recv_client.recv_timeout(Duration::from_secs(1));

        assert!(received.is_ok());
        if let Ok(packet) = received{
            while let Ok(event) = recv_event.recv_timeout(Duration::from_secs(1)) {
                println!("Controller: {:?}", event);
            }
            assert_eq!(packet.pack_type, flood_response_expected.pack_type);
        }
    }

    #[test]
    fn test_send() {
        let (_send_command, _recv_event, _recv_client, recv_server, send_drones, drones) = build_complete_linear_network();
        let mut handlers = Vec::new();
        for (i, mut drone) in drones.into_iter().enumerate() {
            let thread_name = format!("drone_thread_{}", i);
            handlers.push(thread::Builder::new()
                .name(thread_name)  // Assegna il nome al thread
                .spawn(move || {
                    drone.run();  // Esegui il metodo run sul drone
                })
                .unwrap());
        }

        let sended_packet = Packet::new_ack(SourceRoutingHeader::with_first_hop(vec![10,0,1,2,3,4,5,6,7,8,9,11]), 0, 1);
        send_drones[0].send(sended_packet.clone()).unwrap();
        sleep(Duration::from_secs(1));

        let received = recv_server.recv_timeout(Duration::from_secs(1));

        assert!(received.is_ok());
        if let Ok(packet) = received {
            assert_eq!(packet.pack_type, sended_packet.pack_type);
        }
    }


}