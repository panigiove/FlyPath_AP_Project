#[cfg(test)]
mod tests {
    use crate::channel::ChannelManager;
    use crate::comunication::ToUICommunication;
    use crate::network::{NetworkManager, NEW_STATE_GRACE_PERIOD};
    use crossbeam_channel::{unbounded, Receiver};
    use message::NodeEvent;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;
    use wg_2024::network::SourceRoutingHeader;
    use wg_2024::packet::{FloodRequest, Fragment, NodeType, Packet, PacketType};

    fn setup_manager() -> (
        NetworkManager,
        Receiver<NodeEvent>,
        Receiver<ToUICommunication>,
        Receiver<Packet>,
    ) {
        let (tx_ctrl, rx_controller_mock) = unbounded();
        let (tx_ui, rx_user_interface_mock) = unbounded();
        let (_, rx_drone) = unbounded();
        let (_, rx_ctrl) = unbounded();
        let (_, rx_ui) = unbounded();
        let (tx_drone, rx_drone_mock) = unbounded();

        let mock_channels = ChannelManager {
            tx_drone: [(1, tx_drone)].into(),
            tx_ctrl,
            tx_ui,
            rx_drone,
            rx_ctrl,
            rx_ui,
        };

        (
            NetworkManager::new(0, Rc::new(RefCell::new(mock_channels))),
            rx_controller_mock,
            rx_user_interface_mock,
            rx_drone_mock,
        )
    }

    #[test]
    fn test_send_flood_request() {
        let (mut manager, rx_ctrl, rx_ui, rx_drone) = setup_manager();
        manager.send_flood_request();

        let expected_flood_id = 1;
        let expected_packet = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            0,
            FloodRequest::initialize(1, 0, NodeType::Client),
        );

        let ctrl_msg = rx_ctrl.recv_timeout(Duration::from_millis(500)).unwrap();
        let _drone_msg = rx_drone.recv_timeout(Duration::from_millis(500)).unwrap();

        if let NodeEvent::PacketSent(result_packet) = ctrl_msg {
            assert_eq!(result_packet, expected_packet)
        } else {
            panic!()
        }
    }

    #[test]
    fn test_update_network_from_flood_response() {
        let (mut manager, _, rx_ui, _) = setup_manager();
        manager.send_flood_request();

        let flood_request = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            0,
            FloodRequest::initialize(1, 0, NodeType::Client),
        );
        if let PacketType::FloodRequest(mut flood_request) = flood_request.pack_type {
            flood_request.increment(1, NodeType::Drone);
            flood_request.increment(2, NodeType::Server);
            let mut flood_response = flood_request.generate_response(1);
            flood_response.routing_header.increase_hop_index();
            flood_response.routing_header.increase_hop_index();
            if let PacketType::FloodResponse(flood_response) = flood_response.pack_type {
                if let Some(result_server_reach) =
                    manager.update_network_from_flood_response(&flood_response)
                {
                    let expect_server_reach = vec![2];
                    assert_eq!(result_server_reach, expect_server_reach);

                    let expect_route_computed = vec![0, 1, 2];
                    let result_route_computed = manager.state.get_server_path(&2);
                    assert_eq!(expect_server_reach, result_server_reach);
                } else {
                    panic!()
                }
            }
        }
    }

    #[test]
    fn test_send_packet_cached_path() {
        let (mut manager, rx_ctrl, rx_ui, rx_drone) = setup_manager();
        manager
            .state
            .add_link(0, 1, NodeType::Drone, NodeType::Drone, 1);
        manager
            .state
            .add_link(1, 2, NodeType::Drone, NodeType::Server, 1);

        let packet = Packet::new_fragment(
            SourceRoutingHeader::empty_route(),
            0,
            Fragment::from_string(1, 1, "TEST".to_string()),
        );

        let result = manager.send_packet(&packet, &2);
        assert!(result);

        let ctrl_msg = rx_ctrl.recv_timeout(Duration::from_millis(500)).unwrap();
        assert!(matches!(ctrl_msg, NodeEvent::PacketSent(_)));
    }

    #[test]
    fn test_send_packet_triggers_flood_on_missing_route() {
        let (mut manager, _rx_ctrl, rx_ui, _rx_drone) = setup_manager();
        manager.state.creation_time =
            std::time::SystemTime::now() - NEW_STATE_GRACE_PERIOD - Duration::from_secs(1);

        let packet = Packet::new_fragment(
            SourceRoutingHeader::empty_route(),
            0,
            Fragment::from_string(1, 1, "TEST".to_string()),
        );
        let result = manager.send_packet(&packet, &99);
        assert!(!result);
    }
}
