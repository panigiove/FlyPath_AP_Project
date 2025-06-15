use eframe::egui::{Color32, Painter, Pos2, Shape, Stroke};
use egui_graphs::{DisplayEdge, DisplayNode, DrawContext, EdgeProps, Node};
use petgraph::EdgeType;
use petgraph::graph::{IndexType};
use crate::gui_assets::{DrawableNode, NetworkNode};
use crate::utils::AppColor;

// Define a custom edge type with styling properties
#[derive(Debug, Clone)]
pub struct DrawableEdge {
    color: Color32,
    thickness: f32,
    transparency: f32,
}

impl Default for DrawableEdge {
    fn default() -> Self {
        Self {
            color: AppColor::Edge.to_color(), // Default blue color
            thickness: 1.,
            transparency: 1.0,
        }
    }
}

impl From<EdgeProps<DrawableEdge>> for DrawableEdge {
    fn from(props: EdgeProps<DrawableEdge>) -> Self {
        DrawableEdge {
            color: props.payload.color,
            transparency: props.payload.transparency,
            thickness: props.payload.thickness,
        }
    }
}

impl<Ty: EdgeType, Ix: IndexType> DisplayEdge<NetworkNode, DrawableEdge, Ty, Ix, DrawableNode> for DrawableEdge {
    fn shapes(
        &mut self,
        start: &Node<NetworkNode, DrawableEdge, Ty, Ix, DrawableNode>,
        end: &Node<NetworkNode, DrawableEdge, Ty, Ix, DrawableNode>,
        ctx: &DrawContext<'_>,
    ) -> Vec<Shape> {
        let dir = (end.props().location - start.props().location).normalized();
        let start_position = <DrawableNode as DisplayNode<NetworkNode, DrawableEdge, Ty, Ix>>::closest_boundary_point(start.display(), dir);
        let end_position = <DrawableNode as DisplayNode<NetworkNode, DrawableEdge, Ty, Ix>>::closest_boundary_point(end.display(), -dir);

        let start = ctx.meta.canvas_to_screen_pos(start_position);
        let end = ctx.meta.canvas_to_screen_pos(end_position);
        let painter: &Painter = ctx.painter;
        let stroke = Stroke {
            width: self.thickness,
            color: self.color,
        };

        let line = Shape::line_segment([
                                           start,
                                           end,
                                       ], stroke);

        painter.add(line.clone());

        vec![line]
    }

    fn update(&mut self, _state: &EdgeProps<DrawableEdge>) {
        // Update edge properties if needed (e.g., dynamic thickness/color changes)
    }

    fn is_inside(
        &self,
        start: &Node<NetworkNode, DrawableEdge, Ty, Ix, DrawableNode>,
        end: &Node<NetworkNode, DrawableEdge, Ty, Ix, DrawableNode>,
        pos: Pos2,
    ) -> bool {
        let start_pos = &start.props().location;
        let end_pos = &end.props().location;

        let d = (*end_pos - *start_pos).normalized();
        let v = pos - *start_pos;
        let projection = d.dot(v);
        let projected_point = *start_pos + d * projection;

        (pos.distance(projected_point) <= self.thickness * 0.5)
            && (projection >= 0.0 && projection <= start_pos.distance(*end_pos))
    }
}