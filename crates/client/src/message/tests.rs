#[cfg(test)]
mod tests {
    use crate::channel::ChannelManager;
    use crate::comunication::ToUICommunication;
    use crate::message::MessagerManager;
    use crossbeam_channel::{unbounded, Receiver};
    use message::{NodeEvent, RecvMessageWrapper};
    use std::cell::RefCell;
    use std::rc::Rc;
    use wg_2024::packet::{Fragment, Packet};

    fn setup_manager() -> (
        MessagerManager,
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
            MessagerManager::new(0, Rc::new(RefCell::new(mock_channels))),
            rx_controller_mock,
            rx_user_interface_mock,
            rx_drone_mock,
        )
    }

    #[test]
    fn test_receive_multi_fragments_same_session() {
        let (mut manager, _, _, _) = setup_manager();
        let fragment = Fragment::from_string(1, 1, "TEST".to_string());
        let session = 1;
        let sid_a = 1;
        let sid_b = 2;
        assert!(manager.save_received_message(fragment.clone(), session, sid_a));
        assert!(manager.rcv_wrapper.contains_key(&(session, sid_a)));
        assert!(manager.save_received_message(fragment.clone(), session, sid_b));
        assert!(manager.rcv_wrapper.contains_key(&(session, sid_b)));
    }
}
