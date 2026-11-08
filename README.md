# Zenith GUI

GUI toolkit on top of [`bevy_ecs`](https://crates.io/crates/bevy_ecs): the UI tree is an ECS
world, widgets are components, interaction is entity events + observers.

| crate | what |
|---|---|
| `zenith-gui-core` | minimal core: `ZenithApp` (World + schedules), flexbox layout, pointer hit-testing and drag events, `DrawList` + `Renderer` trait, text-measure trait, basic widgets (`Panel`, `Label`, `Button`) |
| `zenith-gui-wgpu` | real `wgpu` backend implementing `Renderer`: SDF rounded rects/borders, anti-aliased lines and béziers, `glyphon` text, scissor clipping, HiDPI; window-surface and headless (offscreen) targets |
| `zenith-gui-widgets` | advanced widgets: imnodes-style **node editor** (nodes, pins, links, pan), `Slider`, `Checkbox`, `ProgressBar` |
| `zenith-gui-app` | `winit` window runner (like `dear-app`) + `HeadlessApp` for snapshot tests |

```rust
use zenith_gui_app::prelude::*;

fn main() -> Result<(), AppError> {
    ZenithWindow::new("Hello")
        .size(800.0, 600.0)
        .setup(|app| {
            let root = app.world.spawn((Panel, Style { padding: UiRect::all(16.0), gap: 8.0, ..Default::default() })).id();
            app.world.spawn((button("Click me"), ChildOf(root)));
            app.add_observer(|c: On<Clicked>| println!("clicked {:?}", c.entity));
        })
        .run()
}
```

> Your crate needs `bevy_ecs = "0.20"` as a direct dependency (bevy's derive macros resolve the
> crate by name); everything else is re-exported through `zenith_gui_app::prelude`.

```
cargo run -p zenith-gui-app --example demo      # window: controls + node editor
```

## Frame

`ZenithApp::update` runs `Startup` once, then `Main` every frame, ordered by `UiSet`:
`Input → Interaction → Update → Layout → PreRender → Render → Cleanup`.
The window layer writes `Viewport` / `PointerState`; `Render` extracts a paint-ordered `DrawList`;
the `Renderer` draws it.

## Testing UIs without a window

`HeadlessApp` renders offscreen (needs any Vulkan/Metal/DX12/GL adapter; software ones such as
lavapipe work) and can save PNGs. GPU tests skip themselves when no adapter exists.
`ZENITH_SNAPSHOT_DIR=out cargo test` writes the PNGs.

## Not done yet

- Keyboard / text input, scrolling and zoom, multi-touch, cursor icons, IME.
- Layout: no wrap, min/max sizes. Clipping is rectangular (no rounded clips).
- Node editor: no selection / box-select / minimap; links are removed by despawning the `Link`.
- Redraws continuously (vsync-throttled) instead of on demand.
