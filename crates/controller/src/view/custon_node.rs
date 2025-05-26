use egui_graphs::NodeProps;
use wg_2024::network::NodeId;
use crate::utility::NodeType;

#[derive(Clone)]
pub struct NodeCustom{
    id: NodeId,
    node_type: NodeType,
    size_x: f32,
    size_y: f32
}

// impl<N: Clone> From<NodeProps<N>> for NodeCustom {
//     fn from(node_props: NodeProps<N>) -> Self {
//         Self {
//             label: node_props.label.clone(),
//             loc: node_props.location(),
// 
//             size_x: 0.,
//             size_y: 0.,
//         }
//     }
// }
