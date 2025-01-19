// TODO: network discovery protocol, da mandare per inizializzare poi ogni tot ms e poi per ogni nack
// TODO: fragmentation of high level messages
// TODO: handle ACK, NACK

use std::collections::HashMap;

pub struct Client{
    adj_list: HashMap<u8, Vec<u8>>,
}


impl Client {
    pub fn new() -> Self {
        Self{
            adj_list: HashMap::new(),
        }
    }
}