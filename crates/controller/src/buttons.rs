use std::sync::mpsc::{Receiver, Sender};
use crossbeam_channel::select_biased;
use eframe::App;
use egui::Context;
use wg_2024::network::NodeId;
use crate::utility::{ButtonEvent, ButtonsMessages};

struct ButtonWindow{
    pub reciver_node_clicked: Receiver<NodeId>, //riceve dal grafo il nodo che è stato premuto
    pub sender_button_event: Sender<ButtonEvent>, //invia al controller le azioni da fare
    pub sender_buttom_messages: Sender<ButtonsMessages>, // dice al grafo quando deve deselezionare un nodo e quando ne può selezionare più di uno
    //these two are the nodes that are selected at the exact instant we are working
    pub node_id1: Option<NodeId>, //entrambi inisializzati a None
    pub node_id2:Option<NodeId>,
    pub is_multiple_selection_allowed: bool //inizializzato a false
}

impl Default for ButtonWindow {
    fn default() -> Self {
        Self { counter: 0 }
    }
}

impl App for ButtonWindow{
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame){
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Pulsanti egui");


            // Visualizza il contatore
            ui.label(format!("Clic totali: {}", self.counter));

            if ui.button("Add a new drone").clicked() {
                //TODO
            }

            if ui.button("Add a new connection with ...").clicked() {
                //TODO
            }

            if ui.button("Remove the connection with ...").clicked() {
                //TODO
            }

            if ui.button("Change the packet drop rate").clicked() {
                //TODO
            }

            if ui.button("Let the drone crash").clicked() {
                //TODO
            }

            // Pulsante con stile personalizzato
            let custom_button = ui.add_sized(
                [100.0, 40.0],
                egui::Button::new("Grande!").fill(egui::Color32::GOLD)
            );

            if custom_button.clicked() {
                ui.label("Hai cliccato il pulsante grande!");
            }
        });
    }
}

impl ButtonWindow{
    pub fn new() -> Self{
        //TODO!
    }

    pub fn run(&mut self){
        self.inizialize_graph();

        loop{
            select_biased!{
                recv(self.reciver_node_clicked) -> command =>{
                    if let Ok(command) = command{
                        self.node_clicked_handler(command);
                    }
                }
                default => {
                    // for (_, reciver) in self.receive_event.clone(){
                    //     if let Ok(event) = reciver.try_recv(){
                    //         self.event_handler(event);
                    //     }
                    // }
                }
            }
            // if let Ok(command) = self.reciver_buttom_messages.try_recv(){
            //
            // }
            // if let Ok(command) = self.ui_receiver.try_recv() {
            //     self.ui_command_handler(command);
            //     continue;
            // }
            //
            // for (_, i) in self.receive_event.clone() {
            //     if let Ok(event) = i.try_recv() {
            //         self.event_handler(event);
            //     }
            // }
            //
            // // Piccola pausa per evitare un ciclo troppo intenso
            std::thread::yield_now();
        }
    }

    pub fn node_clicked_handler(&mut self, node_id: NodeId){
        //teoricamente attivato dopo che noi abbiamo selezionato il primo nodo -> perchè successivamente si potrà selezionare il tasto per aggiungere un nuovo arco
        if self.is_multiple_selection_allowed{
            self.node_id2 = Some(node_id);
        }

        else{
            self.node_id1 = Some(node_id);
        }
        // if self.node_id1.is_none(){
        //     self.node_id1 = Some(node_id);
        // }
    }
}