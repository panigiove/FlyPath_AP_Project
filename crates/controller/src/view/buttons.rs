// use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
// use eframe::App;
// use egui::Context;
// use wg_2024::network::NodeId;
// use crate::utility::{ButtonEvent, ButtonsMessages};
// use crate::utility::ButtonEvent::{ChangePdr, Crash, NewDrone};
// 
// struct ButtonWindow{
//     pub reciver_node_clicked: Receiver<NodeId>, //riceve dal grafo il nodo che è stato premuto
//     pub sender_button_event: Sender<ButtonEvent>, //invia al controller le azioni da fare
//     pub sender_buttom_messages: Sender<ButtonsMessages>, // dice al grafo quando deve deselezionare un nodo e quando ne può selezionare più di uno
//     //these two are the nodes that are selected at the exact instant we are working
//     pub node_id1: Option<NodeId>, //entrambi inisializzati a None quando azione fatta dinuovo a none
//     pub node_id2:Option<NodeId>,
//     pub is_multiple_selection_allowed: bool, //inizializzato a false
//     pub pdr_change: Option<f32>, //inizializza a none, quando cambio fatto dinuovo a none
//     pub current_node_clicked: Option<NodeId>,
//     pub current_pdr: Option<f32>
// }
// 
// impl Default for ButtonWindow {
//     fn default() -> Self {
//         let (sender_button_event, receiver_button_event) = unbounded::<ButtonEvent>();
//         let (sender_buttom_messages, receiver_buttom_messages) = unbounded::<ButtonsMessages>();
//         let (sender_node_clicked, receiver_node_clicked) = unbounded::<NodeId>();
// 
// 
//         Self {
//             reciver_node_clicked: receiver_node_clicked,
//             sender_button_event,
//             sender_buttom_messages,
//             node_id1: None,
//             node_id2: None,
//             is_multiple_selection_allowed: false,
//             pdr_change: None,
//             current_node_clicked: None,
//             current_pdr: None,
//         }
//     }
// }
// 
// impl App for ButtonWindow{
//     fn update(&mut self, ctx: &Context, _: &mut eframe::Frame){
//         egui::CentralPanel::default().show(ctx, |ui| {
//             ui.heading("Pulsanti egui");
// 
// 
//             // Visualizza il contatore
//             //ui.label(format!("Clic totali: {}", self.counter));
// 
//             if ui.button("Add a new drone").clicked() {
//                 self.new_drone();
//             }
// 
//             if ui.button("Add a new connection with ...").clicked() {
//                 self.add_new_connection();
//             }
// 
//             if ui.button("Remove the connection with ...").clicked() {
//                 self.remove_connection();
//             }
// 
//             if ui.button("Change the packet drop rate").clicked() {
//                 self.change_pdr();
//             }
// 
//             if ui.button("Let the drone crash").clicked() {
//                 self.let_drone_crash();
//             }
// 
//             // Pulsante con stile personalizzato
//             let custom_button = ui.add_sized(
//                 [100.0, 40.0],
//                 egui::Button::new("Grande!").fill(egui::Color32::GOLD)
//             );
// 
//             if custom_button.clicked() {
//                 ui.label("Hai cliccato il pulsante grande!");
//             }
//         });
//     }
// }
// 
// impl ButtonWindow{
//     pub fn new() -> Self{
//         //TODO!
//         Self{
//             
//         }
//     }
// 
//     pub fn run(&mut self){
//         //self.inizialize_graph();
// 
//         loop{
//             select_biased!{
//                 recv(self.reciver_node_clicked) -> command =>{
//                     if let Ok(command) = command{
//                         self.node_clicked_handler(command);
//                     }
//                 }
//                 default => {
//                     // for (_, reciver) in self.receive_event.clone(){
//                     //     if let Ok(event) = reciver.try_recv(){
//                     //         self.event_handler(event);
//                     //     }
//                     // }
//                 }
//             }
//             // if let Ok(command) = self.reciver_buttom_messages.try_recv(){
//             //
//             // }
//             // if let Ok(command) = self.ui_receiver.try_recv() {
//             //     self.ui_command_handler(command);
//             //     continue;
//             // }
//             //
//             // for (_, i) in self.receive_event.clone() {
//             //     if let Ok(event) = i.try_recv() {
//             //         self.event_handler(event);
//             //     }
//             // }
//             //
//             // // Piccola pausa per evitare un ciclo troppo intenso
//             std::thread::yield_now();
//         }
//     }
// 
//     pub fn node_clicked_handler(&mut self, node_id: NodeId){
//         //teoricamente attivato dopo che noi abbiamo selezionato il primo nodo -> perchè successivamente si potrà selezionare il tasto per aggiungere un nuovo arco
//         if self.is_multiple_selection_allowed{
//             self.node_id2 = Some(node_id);
//         }
// 
//         else{
//             self.node_id1 = Some(node_id);
//         }
//         // if self.node_id1.is_none(){
//         //     self.node_id1 = Some(node_id);
//         // }
//     }
// 
//     pub fn new_drone(&mut self){
//         if let Some(id) = self.current_node_clicked{
//             if let Some(pdr) = self.current_pdr{
//                 self.sender_button_event.send(NewDrone(id, pdr));  //TODO vedere come gestire
//             }
//         }
//         //deselezionare bottone?
//     }
// 
//     pub fn add_new_connection(&mut self){
//         self.is_multiple_selection_allowed = true;
//         //magari togliamo gli altri pulsanti finchè non usciamo o scegliamo l'altro nodo
//     }
// 
//     pub fn remove_connection(&mut self){
//         self.is_multiple_selection_allowed = true;
//     }
// 
//     pub fn change_pdr(&mut self){
//         if let Some(node) = self.node_id1{
//             //TODO capire quando l'utente inserisce pdr nuovo -> devo far comparire una finestra per l'interimento
//             if let Some(new_pdr) = self.pdr_change{
//                 self.sender_button_event.send(ChangePdr(node, new_pdr)); //vedere cosa fare con il result
//             }
//         }
//     }
// 
//     pub fn let_drone_crash(&self){
//         if let Some(node) = self.node_id1{
//             self.sender_button_event.send(Crash(node)); //capire cosa fare con il resutl
//         }
//     }
// }