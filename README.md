# FlyPath_AP_Project
This is our project for the Advance Programming course

It's composed by four main component:
-Controller
-Client
-Server
-Initializer

# Controller

# Client

# Server
The chat server implementation is Giovanni Panighel's individual contribution, and it provides the basic needs for client to communicate from each other.

## Message Handling
Once a `ChatRequest` message is received, the server will perform various action, based on the content of the message:
 - `ClientList`: will provide the list of the client registred to the chat services and will send back a `ClientList(Vec<NodeId>)`
 - `Register(NodeId)`: will add the client with `NodeId` to the chat services
 - `MessageFrom(from, to , message)`: will send to the client with id `to` the `message` from the client with id `from` via the `MessageFrom(to, message)`

If a client attempt to retrieve the `ClientList` or send a `MessageFrom` while it or the client addressee of the `MessageFrom` are not registered to the chat server, the server will responde with a `ErrorWrongClientId()`

all responses will be send encapsulated inside a `ChatResponse` message.

## Network Management
Our chat server has a component called `NetworkManager` that provide the resources and functionality to manage the network topology known to the server and to calculate the optimal path from the server to a known client.

Every drone in the topology has two parameters that indicate respectively the amount of packet successfully sent through and the total  

