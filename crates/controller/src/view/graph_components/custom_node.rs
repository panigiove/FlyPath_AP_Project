use eframe::egui::{Color32, Shape, Vec2, Rect, Rounding, Stroke};
use eframe::emath::Pos2;
use egui_graphs::{DisplayNode, DrawContext, NodeProps};
use petgraph::{EdgeType, stable_graph::IndexType};
use crate::gui_assets::{NetworkNode, NodeType};
use crate::gui_assets::drawable_edge::DrawableEdge;
use crate::utils::AppColor;
use crate::view::graph_components::custom_edge::DrawableEdge;

#[derive(Clone)]
pub struct DrawableNode {
    payload: NetworkNode,
    label: String,
    loc: Pos2,
    dragged: bool,
    size: f32,
}
fn server_shape(ctx: &DrawContext, rect: Rect, color: Color32, server_id: &str) -> Vec<Shape> {
    let mut shapes = Vec::new();

    let server_width = rect.width() + 10.;
    let server_height = rect.height() / 3.0;
    let spacing = 3.0; // Space between rectangles
    let circle_radius = server_height * 0.2;

    for i in 0..4 {
        let y_offset = rect.top() + (i as f32 * (server_height + spacing));

        // Draw rectangle
        let server_rect = Rect {
            min: Pos2::new(rect.left(), y_offset),
            max: Pos2::new(rect.left() + server_width, if i < 3 { y_offset + server_height } else { y_offset + server_height * 2. }),
        };
        shapes.push(Shape::rect_filled(server_rect, Rounding::ZERO, color));

        if i < 3 {
            // Draw circle (indicator light)
            let circle_center = Pos2::new(rect.left() + circle_radius * 2.5, y_offset + server_height / 2.0);
            shapes.push(Shape::circle_filled(circle_center, circle_radius, AppColor::Surface.to_color()));
        } else {
            let mut font_id = eframe::egui::TextStyle::Body.resolve(&ctx.ctx.style());
            font_id.size = 12.;
            let center = Pos2::new(rect.left() + (server_width / 2.0), y_offset + server_height);

            // Add the text
            let text_shape = Shape::text(
                &ctx.ctx.fonts(|f| f.clone()),
                center,
                eframe::egui::Align2::CENTER_CENTER,
                server_id,
                font_id,
                AppColor::Surface.to_color(),
            );
            shapes.push(text_shape);
        }
    }

    shapes
}

fn client_shape(ctx: &DrawContext, rect: Rect, color: Color32, client_id: &str) -> Vec<Shape> {
    let mut shapes = Vec::new();

    let width = rect.width() * 1.5;
    let height = rect.height() * 1.5;

    // Define proportions
    let spacing = 3.0;
    let case_width = width * 0.3;  // Tower is 30% of total width
    let case_height = height * 0.9; // Tower is 70% of total height
    let monitor_width = width;
    let monitor_height = height * 0.8;
    let stand_height = height * 0.1;
    let circle_radius = case_width * 0.08;

    // **Tower (Left Rectangle)**
    let case_rect = Rect {
        min: Pos2::new(rect.left(), rect.top()),
        max: Pos2::new(rect.left() + case_width, rect.top() + case_height),
    };
    shapes.push(Shape::rect_filled(case_rect, Rounding::ZERO, color));

    // **Power Button (Circle on the Tower)**
    let circle_center = Pos2::new(case_rect.center().x, case_rect.top() + case_height * 0.1);
    shapes.push(Shape::circle_filled(circle_center, circle_radius, AppColor::Surface.to_color()));

    // **Monitor (Right Rectangle)**
    let monitor_left = rect.left() + case_width + spacing;
    let monitor_rect = Rect {
        min: Pos2::new(monitor_left, rect.top()),
        max: Pos2::new(monitor_left + monitor_width, rect.top() + monitor_height),
    };
    shapes.push(Shape::rect_filled(monitor_rect, Rounding::ZERO, color));

    // **Screen (Smaller White Rectangle Inside the Monitor)**
    let screen_rect = Rect {
        min: monitor_rect.min + Vec2::new(monitor_width * 0.1, monitor_height * 0.1),
        max: monitor_rect.max - Vec2::new(monitor_width * 0.1, monitor_height * 0.1),
    };
    shapes.push(Shape::rect_filled(screen_rect, Rounding::ZERO, AppColor::Surface.to_color()));

    // **Monitor Stand**
    let stand_rect = Rect {
        min: Pos2::new(monitor_rect.center().x - case_width * 0.2, monitor_rect.max.y),
        max: Pos2::new(monitor_rect.center().x + case_width * 0.2, monitor_rect.max.y + stand_height),
    };
    shapes.push(Shape::rect_filled(stand_rect, Rounding::ZERO, color));

    // CLIENT ID
    let mut font_id = eframe::egui::TextStyle::Body.resolve(&ctx.ctx.style());
    font_id.size = 12.;
    let center = Pos2::new(screen_rect.left() + (screen_rect.width() / 2.0), screen_rect.top() + (screen_rect.height() / 2.0));

    // Add the text
    let text_shape = Shape::text(
        &ctx.ctx.fonts(|f| f.clone()),
        center,
        eframe::egui::Align2::CENTER_CENTER,
        client_id,
        font_id,
        color,
    );
    shapes.push(text_shape);
    shapes
}

fn drone_shape(ctx: &DrawContext, rect: Rect, color: Color32, drone_id: &str, frame: f32) -> Vec<Shape> {
    let mut shapes = Vec::new();

    let width = rect.width();
    let height = rect.height();

    // Define proportions
    let body_width = width * 0.6;
    let body_height = height * 0.7;
    let arm_length = width * 0.4;
    let arm_thickness = width * 0.15;
    let propeller_length = arm_thickness * 5.0;

    // **Central Body (Rectangle)**
    let body_rect = Rect {
        min: Pos2::new(rect.center().x - body_width / 2.0, rect.center().y - body_height / 2.0),
        max: Pos2::new(rect.center().x + body_width / 2.0, rect.center().y + body_height / 2.0),
    };
    shapes.push(Shape::rect_filled(body_rect, Rounding::from(5.), color));

    // **Arms (X-shape)**
    let arm_positions = [
        (-arm_length * 1.5, -arm_length * 1.5), // Top-left
        (arm_length * 1.5, -arm_length * 1.5),  // Top-right
        (-arm_length * 1.5, arm_length * 1.5),  // Bottom-left
        (arm_length * 1.5, arm_length * 1.5),   // Bottom-right
    ];

    for (count, (dx, dy)) in (0_u8..).zip(arm_positions.iter()) {
        let arm_start = Pos2::new(rect.center().x, rect.center().y);
        let arm_end = Pos2::new(rect.center().x + dx, rect.center().y + dy);
        shapes.push(Shape::line_segment([arm_start, arm_end], Stroke::new(arm_thickness, color)));

        // **Rotating Propellers**
        let angle = if count < 2 { frame * 0.3 } else {frame * -0.3 };// Rotation speed (adjust as needed)
        let cos_theta = angle.cos();
        let sin_theta = angle.sin();

        // Compute rotated propeller positions
        let prop_x1 = -propeller_length / 2.0;
        let prop_y1 = 0.0;
        let prop_x2 = propeller_length / 2.0;
        let prop_y2 = 0.0;

        let rotated_x1 = prop_x1 * cos_theta - prop_y1 * sin_theta;
        let rotated_y1 = prop_x1 * sin_theta + prop_y1 * cos_theta;
        let rotated_x2 = prop_x2 * cos_theta - prop_y2 * sin_theta;
        let rotated_y2 = prop_x2 * sin_theta + prop_y2 * cos_theta;

        let prop_start = Pos2::new(arm_end.x + rotated_x1, arm_end.y + rotated_y1);
        let prop_end = Pos2::new(arm_end.x + rotated_x2, arm_end.y + rotated_y2);
        shapes.push(Shape::line_segment([ prop_start, prop_end ], Stroke::new(arm_thickness * 0.8, color)));

        shapes.push(Shape::circle_filled(arm_end, 2., color));

        // Draw the text (label) if it exists
        let mut font_id = eframe::egui::TextStyle::Body.resolve(&ctx.ctx.style());
        font_id.size = 12.;
        let center = Pos2::new(rect.left() + (rect.width() / 2.0), rect.top() + (rect.height() / 2.0));

        let text_shape = Shape::text(
            &ctx.ctx.fonts(|f| f.clone()),
            center,
            eframe::egui::Align2::CENTER_CENTER,
            drone_id.to_string(),
            font_id,
            AppColor::Surface.to_color(),
        );
        shapes.push(text_shape);
    }

    shapes
}


impl From<NodeProps<NetworkNode>> for DrawableNode {
    fn from(value: NodeProps<NetworkNode>) -> Self {
        Self {
            payload: value.payload.clone(),
            label: value.label.clone(),
            loc: value.location,
            dragged: value.dragged,
            size: 15.,
        }
    }
}

impl<Ty: EdgeType, Ix: IndexType> DisplayNode<NetworkNode, DrawableEdge, Ty, Ix> for DrawableNode {
    // We assume that our custom node is a circle, so the point will always be at the same distance
    fn closest_boundary_point(&self, dir: Vec2) -> Pos2 {
        let normalized_dir = dir.normalized();
        self.loc + normalized_dir * self.size
    }

    fn shapes(&mut self, ctx: &DrawContext) -> Vec<Shape> {
        let center = ctx.meta.canvas_to_screen_pos(self.loc);
        let size = ctx.meta.canvas_to_screen_size(self.size);
        let color = match self.payload.node_type {
            NodeType::Drone => AppColor::Drone.to_color(),
            NodeType::Client => AppColor::Client.to_color(),
            NodeType::Server => AppColor::Server.to_color(),
        };

        let shape: Shape;
        match self.payload.node_type {
            NodeType::Server => shape = Shape::from(server_shape(ctx, Rect::from_center_size(center, Vec2::new(size, size)), color, &self.payload.id.to_string())),
            NodeType::Client => shape = Shape::from(client_shape(ctx, Rect::from_center_size(center, Vec2::new(size, size)), color, &self.payload.id.to_string())),
            NodeType::Drone => {
                shape = Shape::from(drone_shape(ctx, Rect::from_center_size(center, Vec2::new(size, size)), color, &self.payload.id.to_string(), self.payload.frame));
                self.payload.frame += 1.;
            }
        }
        let shapes = vec![shape];

        shapes
    }

    fn update(&mut self, state: &NodeProps<NetworkNode>) {
        self.label = state.label.clone();
        self.loc = state.location;
        self.dragged = state.dragged;
    }

    fn is_inside(&self, pos: Pos2) -> bool {
        // Calculate the squared distance between the point and the center
        let dx = pos.x - self.loc.x;
        let dy = pos.y - self.loc.y;
        let distance_squared = dx * dx + dy * dy;

        // Check if the squared distance is less than or equal to the squared radius
        distance_squared <= self.size * self.size
    }
}