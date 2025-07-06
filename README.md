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

This component interacts with the entire network. Its UI is composed of three sections:

### Network Graph Panel
This area allows direct interaction with the network. The network is represented as a graph, and each node has a label corresponding to its `NodeId`.  
To interact with the network, you must first click on one or two nodes, or an edge.

### Network Controls Panel
This section contains a list of buttons that become active after selecting some graph components.

### Messages Panel
This area displays feedback about what is happening in the network.

## Server
The chat server implementation is Giovanni Panighel's individual contribution, and it provides the basic needs for client to communicate from each other.

### Message Handling
Once a `ChatRequest` message is received, the server will perform various action, based on the content of the message:
 - `ClientList`: will provide the list of the client registred to the chat services and will send back a `ClientList(Vec<NodeId>)`
 - `Register(NodeId)`: will add the client with `NodeId` to the chat services
 - `MessageFrom(from, to , message)`: will send to the client with id `to` the `message` from the client with id `from` via the `MessageFrom(to, message)`

If a client attempt to retrieve the `ClientList` or send a `MessageFrom` while it or the client addressee of the `MessageFrom` are not registered to the chat server, the server will responde with a `ErrorWrongClientId()`

all responses will be send encapsulated inside a `ChatResponse` message.

### Network Management
Our chat server has a component called `NetworkManager` that provide the resources and functionality to manage the network topology known to the server and to calculate the optimal path from the server to a known client.

Every drone in the topology has two parameters that indicate respectively the amount of packet successfully sent through and the total  

