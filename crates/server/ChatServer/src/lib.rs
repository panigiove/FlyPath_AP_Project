use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Receiver, Sender};
use crossbeam_channel::select_biased;
use wg_2024::controller::{DroneCommand, DroneEvent};
use message::*;
use wg_2024::network::*;
use wg_2024::packet::{NodeType, Packet, PacketType};

/*pub trait Server {
    type RequestType: Request;
    type ResponseType: Response;

    fn compose_message(
        source_id: NodeId,
        session_id: u64,
        raw_content: String,
    ) -> Result<Message<Self::RequestType>, String> {
        let content = Self::RequestType::from_string(raw_content)?;
        Ok(Message {
            session_id,
            source_id,
            content,
        })
    }

    fn on_request_arrived(&mut self, source_id: NodeId, session_id: u64, raw_content: String) {
        if raw_content == "ServerType" {
            let _server_type = Self::get_sever_type();
            // send response
            return;
        }
        match Self::compose_message(source_id, session_id, raw_content) {
            Ok(message) => {
                let response = self.handle_request(message.content);
                self.send_response(response);
            }
            Err(str) => panic!("{}", str),
        }
    }

    fn send_response(&mut self, _response: Self::ResponseType) {
        // send response
    }

    fn handle_request(&mut self, request: Self::RequestType) -> Self::ResponseType;

    fn get_sever_type() -> ServerType;
}*/

#[derive(Clone, Debug)]
pub struct ChatServer{
    pub id: NodeId,
    pub controller_send: Sender<DroneEvent>,
    pub controller_recv: Receiver<DroneCommand>,
    pub packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    pub buffer: Vec<Packet>,

    pub topology: Vec<Vec<NodeId>>,
    pub 
}

impl ChatServer{
    fn new(
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Self {
        Self {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            buffer: vec![],
            topology: vec![],
        }
    }

    fn run(&mut self){
        loop{
            select_biased!{
                revc(self.contoller_recv.recv) -> cmd =>{
                    if let Ok(cmd) = cmd {
                        match cmd{

                        }
                    }
                },
                recv(self.packet_recv) -> packet =>{
                    if let Ok(packet) = packet {
                        self.handle_packet(packet);
                    }
                }
            }
        }
    }

    fn handle_packet(&mut self, mut packet: Packet){
        match packet.pack_type{
            PacketType::MsgFragment(fragment) => {

            }
            PacketType::Ack(_) => {}
            PacketType::Nack(_) => {}
            PacketType::FloodRequest(flood_request) => {
                if let Some((last_nodeId, _)) = flood_request.path_trace.last() {
                    let mut updated_flood_request = flood_request.get_incremented(self.id, NodeType::Server);
                    let mut response = updated_flood_request.generate_response(packet.session_id);
                    self.send_packet(&mut response);
                    }
                }
            }
            PacketType::FloodResponse(_) => {}
        }
    }

    fn packet_assembler(){

    }
}

/*impl Server for ChatServer {
    type RequestType = ChatRequest;
    type ResponseType = ChatResponse;

    fn handle_request(&mut self, request: Self::RequestType) -> Self::ResponseType {
        match request {
            ChatRequest::ClientList => {
                println!("Sending ClientList");
                ChatResponse::ClientList(vec![1, 2])
            }
            ChatRequest::Register(id) => {
                println!("Registering {}", id);
                ChatResponse::ClientList(vec![1, 2])
            }
            ChatRequest::SendMessage {
                message,
                to,
                from: _,
            } => {
                println!("Sending message \"{}\" to {}", message, to);
                // effectively forward message
                ChatResponse::MessageSent
            }
        }
    }

    fn get_sever_type() -> ServerType {
        ServerType::Chat
    }
}*/


