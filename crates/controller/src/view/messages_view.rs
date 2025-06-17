use crossbeam_channel::{select, select_biased, Receiver, Sender, unbounded};
use eframe::egui::Color32;
use eframe::Frame;
use egui::{Context, TopBottomPanel};
use egui::RichText;
use crate::utility::{ButtonEvent, GraphAction, NodeType, MessageType};

pub struct MessagesWindow{
    pub messages_reciver: Receiver<MessageType>, //the controller_handler sends the messages to print
    pub log: Vec<MessageType>, //facciamo che dopo un tot di stringhe quelle più vecchie vengono eliminate
}

impl MessagesWindow{
    pub fn new(receiver: Receiver<MessageType>) -> Self{
        Self{
            messages_reciver: receiver,
            log: Vec::new(),
        }
    }
    
    pub fn run(&mut self){
        loop{
            self.handle_incoming_messages();
            
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    pub fn update(&mut self, ctx: &egui::Context){
        self.handle_incoming_messages();

        egui::TopBottomPanel::bottom("Feedback").exact_height(150.0).show(ctx, |ui|{
            ui.add_space(10.0);
            ui.label("terminale");
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui|{
                for line in &self.log{
                    match line{
                        //     Error(String),
                        //     Ok(String),
                        //     PacketSent(String),
                        //     PacketDropped(String),
                        //     Info(String),
                        //     //TODO vedere se aggiungere un tipo di messaggi per il drone
                        // }
                        MessageType::Error(t) => {
                            let text = RichText::new(t).color(egui::Color32::from_rgb(234, 162, 124)); //orange
                            ui.label(text);
                        }
                        MessageType::Ok(t) => {
                            // Verde per i messaggi OK
                            let text = RichText::new(t).color(egui::Color32::from_rgb(232, 187, 166)); //pink
                            ui.label(text);
                        }
                        MessageType::PacketSent(t) =>{
                            let text = RichText::new(t).color(egui::Color32::from_rgb(14, 137, 145)); //blue
                            ui.label(text);
                        }
                        MessageType::PacketDropped(t) => {
                            let text = RichText::new(t).color(egui::Color32::from_rgb(12, 49, 59)); //dark blu-green
                            ui.label(text);
                        }
                        MessageType::Info(t) => {
                            let text = RichText::new(t).color(egui::Color32::from_rgb(141, 182, 188)); //same color as icons
                            ui.label(text);
                        }
                        _ => {
                            // Colore neutro per messaggi non classificati
                            let text = RichText::new("Messaggio non classificato").color(egui::Color32::GRAY);
                            ui.label(text);
                        }
                    }
                }

                // Auto-scroll verso il basso per vedere sempre gli ultimi messaggi
                let (_, bottom) = ui.allocate_space(egui::vec2(0.0, 10.0));
                ui.scroll_to_rect(bottom, Some(egui::Align::BOTTOM));
            });
            ui.add_space(10.0);
        });
    }

    pub fn handle_incoming_messages(&mut self) {
        while let Ok(message) = self.messages_reciver.try_recv() {
            self.log.push(message);
            if self.log.len() > 1000 {
                self.log.remove(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utility::MessageType;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_handle_incoming_messages_trims_log() {
        let (sender, receiver) = unbounded();
        let mut window = MessagesWindow::new(receiver);

        // Inviamo 1005 messaggi
        for i in 0..1005 {
            let _ = sender.send(MessageType::Ok(format!("Messaggio {}", i)));
        }

        // Chiamiamo handle_incoming_messages
        window.handle_incoming_messages();

        // Verifica: log deve avere 1000 messaggi
        assert_eq!(window.log.len(), 1000);
        assert_eq!(
            match &window.log[0] {
                MessageType::Ok(msg) => msg,
                _ => panic!("Unexpected message type"),
            },
            "Messaggio 5"
        );
    }


    #[test]
    fn test_handle_incoming_error_messages_trims_log() {
        let (sender, receiver) = unbounded();
        let mut window = MessagesWindow::new(receiver);

        // Inviamo 1005 messaggi di tipo Error
        for i in 0..1005 {
            let _ = sender.send(MessageType::Error(format!("Errore {}", i)));
        }

        // Chiamiamo handle_incoming_messages
        window.handle_incoming_messages();

        // Verifica: log deve avere 1000 messaggi
        assert_eq!(window.log.len(), 1000);
        assert_eq!(
            match &window.log[0] {
                MessageType::Error(msg) => msg,
                _ => panic!("Unexpected message type"),
            },
            "Errore 5"
        );
    }

    #[test]
    fn test_handle_incoming_messages_empty_log() {
        let (_sender, receiver) = unbounded();
        let mut window = MessagesWindow::new(receiver);

        window.handle_incoming_messages();
        assert!(window.log.is_empty());
    }

    #[test]
    fn test_handle_incoming_messages_order_preserved() {
        let (sender, receiver) = unbounded();
        let mut window = MessagesWindow::new(receiver);

        sender.send(MessageType::Ok("First".to_string())).unwrap();
        sender.send(MessageType::Error("Second".to_string())).unwrap();

        window.handle_incoming_messages();

        assert_eq!(window.log.len(), 2);
        match &window.log[0] {
            MessageType::Ok(msg) => assert_eq!(msg, "First"),
            _ => panic!("Unexpected message type"),
        }
        match &window.log[1] {
            MessageType::Error(msg) => assert_eq!(msg, "Second"),
            _ => panic!("Unexpected message type"),
        }
    }


    #[test]
    fn test_handle_incoming_messages_multiple_trims() {
        let (sender, receiver) = unbounded();
        let mut window = MessagesWindow::new(receiver);

        // Inviamo 3000 messaggi
        for i in 0..3000 {
            let _ = sender.send(MessageType::Ok(format!("Messaggio {}", i)));
        }

        // Chiamiamo handle_incoming_messages
        window.handle_incoming_messages();

        // Verifica: il log contiene solo gli ultimi 1000 messaggi
        assert_eq!(window.log.len(), 1000);
        assert_eq!(
            match &window.log[0] {
                MessageType::Ok(msg) => msg,
                _ => panic!("Unexpected message type"),
            },
            "Messaggio 2000"
        );
        assert_eq!(
            match &window.log[999] {
                MessageType::Ok(msg) => msg,
                _ => panic!("Unexpected message type"),
            },
            "Messaggio 2999"
        );
    }

    #[test]
    fn test_handle_incoming_messages_stress_test() {
        let (sender, receiver) = unbounded();
        let mut window = MessagesWindow::new(receiver);

        // Inviamo 10.000 messaggi velocemente
        for i in 0..10_000 {
            sender.send(MessageType::Error(format!("Errore {}", i))).unwrap();
        }

        // Processiamo i messaggi
        window.handle_incoming_messages();

        // Devono esserci solo gli ultimi 1000 messaggi
        assert_eq!(window.log.len(), 1000);
        assert_eq!(
            match &window.log[0] {
                MessageType::Error(msg) => msg,
                _ => panic!("Unexpected message type"),
            },
            "Errore 9000"
        );
        assert_eq!(
            match &window.log[999] {
                MessageType::Error(msg) => msg,
                _ => panic!("Unexpected message type"),
            },
            "Errore 9999"
        );
    }
}

#[cfg(test)]
mod display_tests {
    use super::*;
    use crate::utility::MessageType;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_message_display_logic() {
        let (sender, receiver) = unbounded();
        let mut window = MessagesWindow::new(receiver);

        // Invia diversi tipi di messaggi
        sender.send(MessageType::Error("Errore di test".to_string())).unwrap();
        sender.send(MessageType::Ok("Operazione riuscita".to_string())).unwrap();

        // Process messages
        window.handle_incoming_messages();

        // Verifica che i messaggi siano stati ricevuti
        assert_eq!(window.log.len(), 2);

        // Verifica il contenuto dei messaggi
        match &window.log[0] {
            MessageType::Error(msg) => assert_eq!(msg, "Errore di test"),
            _ => panic!("Primo messaggio dovrebbe essere Error"),
        }

        match &window.log[1] {
            MessageType::Ok(msg) => assert_eq!(msg, "Operazione riuscita"),
            _ => panic!("Secondo messaggio dovrebbe essere Ok"),
        }
    }

    #[test]
    fn test_message_types_coverage() {
        // Questo test aiuta a identificare se tutti i tipi di MessageType sono gestiti
        let (sender, receiver) = unbounded();
        let mut window = MessagesWindow::new(receiver);

        // Aggiungi qui tutti i tipi di MessageType che hai definito
        sender.send(MessageType::Error("Test error".to_string())).unwrap();
        sender.send(MessageType::Ok("Test ok".to_string())).unwrap();
        // Aggiungi altri tipi se esistono...

        window.handle_incoming_messages();

        // Verifica che tutti i messaggi siano stati processati
        assert!(!window.log.is_empty());
        println!("Messaggi processati: {}", window.log.len());
    }
}
