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

    pub fn update(&mut self, ctx: &egui::Context){
        self.handle_incoming_messages();

        egui::TopBottomPanel::bottom("Feedback").exact_height(150.0).show(ctx, |ui|{
            ui.add_space(10.0);
            ui.label("terminale");
            egui::ScrollArea::vertical().auto_shrink([false; 2]).show(ui, |ui|{
                for line in &self.log{
                    //QUI SISTEMARE COLORE
                    match line{
                        MessageType::Error(t) => {
                            //TODO convertire colori da ff a rgb
                            let text = RichText::new(t).color(egui::Color32::from_rgb(255, 128, 0));
                            ui.label(text);
                        }
                        MessageType::Ok(t) => {
                            //TODO convertire colori da ff a rgb
                            let text = RichText::new(t).color(egui::Color32::from_rgb(255, 128, 0));
                            ui.label(text);
                        }
                        _ =>{
                            
                        }
                    }

                }
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
