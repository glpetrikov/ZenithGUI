use zenith_gui_core::prelude::*;
use zenith_gui_wgpu::prelude::*;
use zenith_gui_widgets::prelude::*;

#[test]
fn button_with_label_produces_ordered_draw_commands() {
    let mut app = ZenithApp::new();
    app.add_plugin(WidgetsPlugin).add_plugin(RenderPlugin);
    let btn = app.world.spawn(button("OK")).id();
    app.update();

    let list = app.world.resource::<DrawList>();
    assert_eq!(list.commands.len(), 2, "button background + label text");
    let DrawCommand::Rect { rect, color } = &list.commands[0] else {
        panic!("bg first")
    };
    assert_eq!(*color, ButtonColors::default().normal);
    // Root button fills the viewport; its label is centered across (padding 8, "OK" = 16px wide).
    assert_eq!(*rect, Rect::new(0.0, 0.0, 800.0, 600.0));
    let DrawCommand::Text { text, rect, .. } = &list.commands[1] else {
        panic!("text second")
    };
    assert_eq!(text, "OK");
    assert_eq!(rect.pos, Vec2::new(392.0, 8.0));
    let _ = btn;
}
