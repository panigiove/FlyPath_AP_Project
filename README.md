# FlyPath_AP_Project

This repository contains the project for the Advanced Programming course, developed by Malandra Martina, Panighel Giovanni, and Ye Daniele.

It is composed of four main components:
- **Client** (Ye Daniele)
- **Controller** (Malandra Martina)
- **Initializer** (All)
- **Server** (Panighel Giovanni)

How to run:

```bash
cargo run -- input.toml 
```
use relative path of file.

## HIGH LEVEL CHAT MESSAGE between CLIENT-SERVER

```rust
pub enum ChatRequest {
    ClientList,
    Register(NodeId),
    SendMessage {
        from: NodeId,
        to: NodeId,
        message: String,
    },
}

pub enum ChatResponse {
    ClientList(Vec<NodeId>),
    MessageFrom { from: NodeId, message: Vec<u8> },
    ErrorWrongClientId(NodeId),
}
```

### Wrappers

```rust
pub struct SentMessageWrapper {
    pub session_id: u64,
    pub destination: NodeId,
    pub total_n_fragments: u64,
    pub acked: HashSet<u64>,
    pub fragments: Vec<Fragment>,

    pub raw_data: String,
}
pub struct RecvMessageWrapper {
    pub session_id: u64,
    pub source: NodeId,
    pub total_n_fragments: u64,
    pub arrived: HashSet<u64>,
    pub fragments: Vec<Option<Fragment>>,

    pub raw_data: String,
}
```

Clients and servers also use two types of fragment `Wrapper`s: one for generated fragments and one for received fragments. These two wrappers help to generate and manage fragments by counting sent, acknowledged, and missing fragments, and provide APIs to convert messages to wrappers and fragments, and vice versa.

## MESSAGE between Client/Server and Controller

```rust
pub enum NodeCommand {
    RemoveSender(NodeId),
    AddSender(NodeId, Sender<Packet>),
    FromShortcut(Packet),
}
pub enum NodeEvent {
    PacketSent(Packet),
    CreateMessage(SentMessageWrapper), // try send message (every times is sended a stream of fragment)
    MessageRecv(RecvMessageWrapper),   // received full message
    ControllerShortcut(Packet),
}
```

## Client

`ChatClient` implements a GUI to chat between multiple instances of clients, using one or more instances of `ChatServer` as middlemen.

### GUI

Implemented using the Rust library `egui` as the GUI engine and its official application framework `eframe` to build a native and high-performance interface.

The Client UI is rendered on a **secondary asynchronous window**. For each client instance, it has its own tab and isolated chats, thanks to the client's `UiState`.

```rust
pub struct UiState {
    input: String,
    current_client: Option<NodeId>,
    client_states: HashMap<NodeId, ClientState>,
}
pub struct ClientState {
    my_id: NodeId,
    current_chat: Option<NodeId>,
    unread_chat: HashSet<NodeId>,

    chat_message: HashMap<NodeId, Vec<(NodeId, String)>>,
    rx_from_worker: Receiver<ToUICommunication>,
    tx_to_worker: Sender<FromUiCommunication>,
}
```

The GUI try to get communication from workers (client threads) every frame of the main window and sends commands to the workers upon each interaction. Communications are handled using `crossbeam_channel`.

### Worker

The Backend and "real" instance of a Client. It manages, sends, and listens to commands from the `Controller`, user interactions from the `GUI`, and packets from other `Nodes`.

#### Network

To achieve high performance, reactivity, and low latency without saturating the network, `NetworkManager` maintains two `NetworkState` instances: one current and one old. It implements state aging, state invalidity, and a grace period.

###### NetworkState

```rust
pub struct NetworkState {
    topology: Graph<NodeId, Weight>,
    id_to_idx: HashMap<NodeId, NodeIndex>,
    start_idx: NodeIndex,
    start_id: NodeId,
    pub server_list: HashSet<NodeId>,
    routing_table: HashMap<NodeId, Vec<NodeId>>, // destination -> path

    creation_time: SystemTime,
    flood_interval: Duration, // default 10 seconds,
    failed_error_count: u8, // default 30%
    failed_drop_count: u8, // default 20%
    error_scale: u32,
    drop_scale: u32,
}
```

`NetworkState`, as its name suggests, is used to save a state of the client's known topology. It contains a cache to save all routes to all known servers. In this way, when a packet is sent, it is not required to perform a Dijkstra calculation. Clients are not added to the topology (only the client itself is), and `Servers` only have entry edges, so they are not considered when a path is elaborated.

It uses the `petgraph` library to create a graph.

- **NetworkState invalidation**

```rust
    pub fn should_flood(&self) -> bool {
        let edge_count = self.topology.edge_count() as u32;

        let error_threshold = (edge_count * self.error_scale / 100).clamp(10, 100) as u8;
        let drop_threshold = (edge_count * self.drop_scale / 100).clamp(5, 50) as u8;

        let elapsed = self
            .creation_time
            .elapsed()
            .unwrap_or(Duration::from_secs(0));

        elapsed > self.flood_interval
            || self.failed_error_count > error_threshold
            || self.failed_drop_count > drop_threshold
    }
```

Aging and invalidation are handled by `should_flood()`. This function checks if the current state is too old or if too many packets have been `Dropped` or too many `ErrorInRouting` events have occurred. The thresholds for these two are elaborated in proportion to the number of edges and are clamped within defined limits. This function is called by the `Worker` by invoking `NetworkState`'s API `should_flood()` at the end of every loop.

#### NetworkManager

```rust
pub struct NetworkManager {
    my_id: NodeId,

    pub state: NetworkState,
    pub old_state: NetworkState,

    channels: Rc<RefCell<ChannelManager>>,
    last_flood: Session,
}
```

The heart of network-related activity:

- Sends `FloodRequest`.
- Updates state after receiving both `FloodRequest` and `FloodResponse` (regardless of whether it's from itself or not, and without checking the `flood_id`).
- Sends packets and events (to `Controller`).

##### Double State and grace period

Before starting a new flooding process, the current state is moved to `old_state`. This allows valid routes from the `old_state` to be used, during the flooding and preventing the packet stagnation. The grace period is the interval during which a new state is still considered valid; if a server is unreachable (no route found), a new flooding should not be initiated. The default grace period is 3 seconds. During this grace period, if a route is missed, the network manager checks if there is a route in the `old_state` and attempts to use it. If sending fails after 3 attempts, the packet is saved inside a buffer. After grace period after a miss a new flooding is stated.

#### MessageManager

```rust
pub struct MessagerManager {
    my_id: NodeId,

    channels: Rc<RefCell<ChannelManager>>,

    pub clients: HashMap<NodeId, HashSet<NodeId>>, // client -> server

    buffer: HashMap<NodeId, Vec<Packet>>, // server -> buffer
    msg_wrapper: HashMap<Session, SentMessageWrapper>,
    rcv_wrapper: HashMap<(Session, NodeId), RecvMessageWrapper>,
    last_session: Session,
}
```

This manager creates a `SentMessageWrapper` from a `ChatRequest`. The `SentMessageWrapper` is a struct that contains `Fragments` and other metadata. It stores these until the entire message is acknowledged. Each `Fragment` from a `NodeId` and a `Session` is stored until clients have received all of them.
Any packets that are not sent are stored in a buffer. Each server has its own buffer, and when a server becomes reachable, all packets within its buffer are sent.

## Controller

This component interacts with the entire network. At its core, the ControllerHandler is responsible for performing the desired actions on the network. Its UI is composed of three sections:

### Network Graph Panel
This area allows direct interaction with the network. The network is represented as a graph, and each node has a label: `NodeType NodeId`.
Nodes can be dragged.
To interact with the network, you must double-click one or two nodes.
To verify that your selection is correct, check the **Selection Info** panel:  
![Selection Info](crates/assets/images_readme/selection_info.png)  
**NOTE**: Due to the fact that the UI is built with egui (which uses frames), selection may occasionally behave unexpectedly. In such cases, it’s highly recommended to clear the selection either by clicking the `Clear All Selections` button or by pressing the `ESC` key on your keyboard.

### Network Control Panel
This section contains a list of buttons that become active after selecting graph components.
Here you can choose which action the controller should perform.
Due to network requirements, you must first select one or two nodes (depending on the action) and then click the corresponding action button.
If two nodes are selected but the action only requires one, the controller will perform the action on the first selected node.

### Messages Panel
This area displays feedback about what is happening in the network. There are five types of messages:  
- **Error**: shown when something goes wrong while executing the selected action.
- **Ok**: shown when the requested action has been successfully completed. 
- **Packet**: shown when the controller receives information about circulating packets from drones.
- **Info**: shown when the controller receives updates about new messages from clients or the server.

## Server
The chat server implementation is Giovanni Panighel's individual contribution, and it provides the functionalities that permit clients to communicate with each other.

The main component of the server, `ChatServer`, stores the connections between the controller and the neighbour drones, as well as two suport struct, `NetworkManager` and `ServerMessageManager`, that provide respectively the services for initialize, store and update the topology known to the server, and the functionality for storing and handling incoming and outgoing message fragments.  

```rust
    pub struct ChatServer {
        pub id: NodeId,
        pub controller_send: Sender<NodeEvent>,
        pub controller_recv: Receiver<NodeCommand>,
        pub packet_recv: Receiver<Packet>,
        pub packet_send: HashMap<NodeId, Sender<Packet>>,
        pub last_session_id: u64,
        pub last_flood_id: 0,
        pub network_manager: NetworkManager,
        pub server_message_manager: ServerMessageManager,
        pub server_buffer: HashMap<NodeId, Vec<Packet>>,
    }
```
### Network Manager

`NetworkManager` stores inside itself a `HashMap<NodeId, (HashSet<NodeId>, TotalSuccesfulPackets, TotalPackets)>` that represent the topology in the form of a adjacency list, all clients detected in the network and all possible route to them, as well as other parameters used to calculate the contition to initiate a new `FloodRequest`.

```rust
    pub struct NetworkManager {
        pub(crate) topology: HashMap<NodeId, (HashSet<NodeId>, TotalSuccesfulPackets, TotalPackets)>,
        pub(crate) routes: HashMap<NodeId, Vec<NodeId>>,
        pub(crate) client_list: HashSet<NodeId>,
        server_id: NodeId,
        pub(crate) n_errors: i64,
        pub(crate) n_dropped: i64,
        flood_interval: Duration,
        start_time: SystemTime,
    }
```
Every node in the topology has two parameters in addition to the `HashSet` of neightbours, `TotalSuccessfulPackets` and `TotalPackes` that represent the total packet successfully (not dropped) passed through the node and the total number of packets passed through the node. They are used to estimate during the calculation of a path which drone has the lower chance of not dropping a packet, attempting the maximum guarantee of delivering it to destination.

The optimal path is computed with Dijkstra's Algorithm, using the probabilities of the nodes of sending a packet (`TotalSuccessfulPackets`/`TotalPackes`) as the weight of the edges of the topology transformed using the negative logarithm of themself. This will make them addable and therefore suitable to operate with the algorithm. The shortest path will correspond to the path with the highest probability of sending the packet to destination, and thus the path with the lowest probability of dropping a packet.

### Messages Manager

`ServerMessageManager` stores inside itself all incoming and outgoing fragments, in addition to the list of clients that requested to be registered in order to use the server functionality to communicate with other registered clients.

```rust
    pub struct ServerMessageManager {
        incoming_fragments: HashMap<(u64, NodeId), RecvMessageWrapper>,
        pub(crate) outgoing_packets: HashMap<u64, SentMessageWrapper>,
        registered_clients: HashSet<NodeId>,
    }
```
Once a `MsgFragment` arrive, it is wrapped inside a `RecvMessageWrapper` and the stored in `incoming_fragments`, and when all fragments are arrived, the message is deserialize in a `ChatRequest`. Based on the content of `ChatRequest` the server will perform various action, based on the content of the message:
 - `ClientList`: will provide the list of the client registred to the chat services and will send back a `ClientList(Vec<NodeId>)`.
 - `Register(NodeId)`: will add the client with `NodeId` to the chat services.
 - `MessageFrom(from, to , message)`: will send to the client with id `to` the `message` from the client with id `from` via the `MessageFrom(to, message)`.

If a client attempt to retrieve the `ClientList` or send a `MessageFrom` while it or the client addressee of the `MessageFrom` are not registered to the chat server, the server will responde with a `ErrorWrongClientId()`.

all responses will be send encapsulated inside a `ChatResponse` message, wrapped in a `SentMessageWrapper` and stored inside `outgoing_packets`.

### Packet Handling

Once a `Packet` arrive, the server will handle it based on `PacketType`:

- `MsgFragment`: `ServerMessageManager` handle it and then all serialized fragment of `ChatResponse` stored in `outgoing_packets` will be sent throght the network.

- `Ack`: the corresponding fragment inside `outgoing_packets` is signed as acked, and `network_manager` will update all weight of the node contained in the `SourceRoutingHeader`.

- `Nack`: the `network_manager` will update all weight of the node contained in the `SourceRoutingHeader` and then proceed to update the number of errors or the number of dropped packet based on the `NackType`. If the nack is a `ErrorInRouting`, the faulty node will be deleted from the topology and all routes generated again. Then the corresponding fragment will be sent again to destination.

- `FloodRequest` and `FloodResponse`: `network_manager` will update the topology of the network and, in the case of `FloodRequest`, generate a `FloodResponse` and send it back in the network.

If an error occur while sending a `Packet`, the number of errors inside `network_manager` is updated and, if it is an `Ack` or a `FloodResponse`, it will be sent to destination via `ControllerShortcut`, otherwise it will be stored in `server_buffer` and tried to be sent again with an updated `SourceRoutingHeader`.

Every time a packet is received, created and sent, it will be notified to the controller. If the server lose the communication channel with the controller, it will panic and end execution.

### Flooding Initialization

Upon spawning, the server will broadcast several `FloodRequest` to the neightbours drones, and it will repeat this procedure every time a max amount of errors or dropped packet are detected, or when a certain interval of time from the last Flooding Initialization has passed.


