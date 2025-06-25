use eframe::egui;

/// Trait principale per componenti UI drawable
pub trait Drawable {
    fn update(&mut self);
    fn render(&mut self, ui: &mut egui::Ui);
    fn needs_continuous_updates(&self) -> bool { false }
    fn component_name(&self) -> &'static str { "Unknown Component" }
}

/// Trait per componenti che specificano dove essere posizionate
pub trait PanelDrawable: Drawable {
    fn preferred_panel(&self) -> PanelType;
    fn preferred_size(&self) -> Option<egui::Vec2> { None }
    fn is_resizable(&self) -> bool { true }
}

/// Tipi di pannelli disponibili
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PanelType {
    Top,
    Bottom,
    Left,
    Right,
    Central,
    Window,
}

/// Layout manager che gestisce tutte le componenti
pub struct LayoutManager {
    pub top_components: Vec<Box<dyn PanelDrawable>>,
    pub bottom_components: Vec<Box<dyn PanelDrawable>>,
    pub left_components: Vec<Box<dyn PanelDrawable>>,
    pub right_components: Vec<Box<dyn PanelDrawable>>,
    pub central_components: Vec<Box<dyn PanelDrawable>>,
    pub window_components: Vec<Box<dyn PanelDrawable>>,
}

impl LayoutManager {
    pub fn new() -> Self {
        Self {
            top_components: Vec::new(),
            bottom_components: Vec::new(),
            left_components: Vec::new(),
            right_components: Vec::new(),
            central_components: Vec::new(),
            window_components: Vec::new(),
        }
    }

    pub fn add_component(&mut self, component: Box<dyn PanelDrawable>) {
        match component.preferred_panel() {
            PanelType::Top => self.top_components.push(component),
            PanelType::Bottom => self.bottom_components.push(component),
            PanelType::Left => self.left_components.push(component),
            PanelType::Right => self.right_components.push(component),
            PanelType::Central => self.central_components.push(component),
            PanelType::Window => self.window_components.push(component),
        }
    }

    pub fn update_all(&mut self) {
        for component in &mut self.top_components { component.update(); }
        for component in &mut self.bottom_components { component.update(); }
        for component in &mut self.left_components { component.update(); }
        for component in &mut self.right_components { component.update(); }
        for component in &mut self.central_components { component.update(); }
        for component in &mut self.window_components { component.update(); }
    }

    pub fn render_all(&mut self, ctx: &egui::Context) {
        // Top panels
        if !self.top_components.is_empty() {
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                for component in &mut self.top_components {
                    component.render(ui);
                    ui.separator();
                }
            });
        }

        // Bottom panels
        if !self.bottom_components.is_empty() {
            let height = self.bottom_components.iter()
                .filter_map(|c| c.preferred_size().map(|s| s.y))
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(200.0);

            egui::TopBottomPanel::bottom("bottom_panel")
                .exact_height(height)
                .show(ctx, |ui| {
                    for component in &mut self.bottom_components {
                        component.render(ui);
                        ui.separator();
                    }
                });
        }

        // Central area with side panels
        egui::CentralPanel::default().show(ctx, |ui| {
            // Left panels
            if !self.left_components.is_empty() {
                let width = self.left_components.iter()
                    .filter_map(|c| c.preferred_size().map(|s| s.x))
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(220.0);

                egui::SidePanel::left("left_panel")
                    .exact_width(width)
                    .show_inside(ui, |ui| {
                        for component in &mut self.left_components {
                            component.render(ui);
                            ui.separator();
                        }
                    });
            }

            // Right panels
            if !self.right_components.is_empty() {
                let width = self.right_components.iter()
                    .filter_map(|c| c.preferred_size().map(|s| s.x))
                    .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap_or(320.0);

                egui::SidePanel::right("right_panel")
                    .exact_width(width)
                    .show_inside(ui, |ui| {
                        for component in &mut self.right_components {
                            component.render(ui);
                            ui.separator();
                        }
                    });
            }

            // Central components
            egui::CentralPanel::default().show_inside(ui, |ui| {
                for component in &mut self.central_components {
                    component.render(ui);
                    ui.separator();
                }
            });
        });

        // Window components
        for component in &mut self.window_components {
            egui::Window::new(component.component_name())
                .show(ctx, |ui| {
                    component.render(ui);
                });
        }
    }

    pub fn needs_repaint(&self) -> bool {
        self.top_components.iter().any(|c| c.needs_continuous_updates()) ||
            self.bottom_components.iter().any(|c| c.needs_continuous_updates()) ||
            self.left_components.iter().any(|c| c.needs_continuous_updates()) ||
            self.right_components.iter().any(|c| c.needs_continuous_updates()) ||
            self.central_components.iter().any(|c| c.needs_continuous_updates()) ||
            self.window_components.iter().any(|c| c.needs_continuous_updates())
    }
}