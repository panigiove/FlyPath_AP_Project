mod utility;

use std::ops::Receiver;
use crate::utility::{ButtonEvent, GraphAction, NodeType, MessageType};

pub struct MessagesWindow{
    pub messages_reciver: Receiver<MessageType>, //the controller_handler sends the messages to print
}