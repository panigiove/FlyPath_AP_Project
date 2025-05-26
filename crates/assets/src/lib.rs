use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use wg_2024::network::NodeId;
use wg_2024::packet::{Fragment, Packet};

use crossbeam_channel::Sender;
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum DroneGroup{
    RustInPeace,
    BagelBomber,
    LockheedRustin,
    RollingDrone,
    RustDoIt,
    RustRoveri,
    Rustastic,
    RustBusters,
    LeDronJames,
    RustyDrones,
}