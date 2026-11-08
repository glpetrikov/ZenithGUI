//! Advanced widgets built on `zenith-gui-core` (which only ships Panel, Label
//! and Button):
//!
//! - [`node_editor`]: an imnodes-style node graph editor (draggable nodes,
//!   pins, drag-to-link bezier links, panning canvas);
//! - [`slider`], [`checkbox`], [`progress`].
//!
//! Add [`WidgetsPlugin`] to the app to enable all of them.

use zenith_gui_core::prelude::*;

pub mod checkbox;
pub mod node_editor;
pub mod progress;
pub mod slider;
pub mod theme;

pub mod prelude {
    pub use crate::WidgetsPlugin;
    pub use crate::checkbox::{Checkbox, Toggled, checkbox};
    pub use crate::node_editor::{
        EditorView, GraphNode, GraphNodeDesc, GraphNodeHandle, GraphPos, Link, LinkCreated,
        LinkRemoved, NodeEditor, Pin, PinKind, spawn_graph_node, spawn_graph_node_in,
    };
    pub use crate::progress::{ProgressBar, progress_bar};
    pub use crate::slider::{Slider, SliderChanged, slider};
}

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut ZenithApp) {
        app.add_plugin(slider::SliderPlugin)
            .add_plugin(checkbox::CheckboxPlugin)
            .add_plugin(progress::ProgressPlugin)
            .add_plugin(node_editor::NodeEditorPlugin);
    }
}
