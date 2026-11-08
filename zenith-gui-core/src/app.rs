//! Minimal application shell around a bevy [`World`].
//!
//! There is intentionally no `bevy_app` dependency: [`ZenithApp`] owns a
//! `World`, a one-shot `Startup` schedule and a per-frame `Main` schedule whose
//! phases are ordered by [`UiSet`]. A windowing backend calls
//! [`ZenithApp::update`] once per frame.

use bevy_ecs::{
    prelude::*,
    schedule::{IntoScheduleConfigs, ScheduleLabel},
    system::{IntoObserverSystem, ScheduleSystem},
};

use crate::{input, interaction, layout, render, text, widgets};

#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZenithSchedule {
    /// Runs once, before the first frame.
    Startup,
    /// Runs every frame.
    Main,
}

/// Ordered phases of a frame (`Main` schedule).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSet {
    /// Backend writes [`PointerState`](crate::input::PointerState), [`Viewport`](crate::input::Viewport), ...
    Input,
    /// Hit-testing against the previous frame's layout; emits `Clicked`, ...
    Interaction,
    /// Application / widget logic.
    Update,
    /// Computes [`ComputedLayout`](crate::node::ComputedLayout).
    Layout,
    /// Widgets write [`CustomDraw`](crate::render::CustomDraw) from the finished layout.
    PreRender,
    /// Extracts the [`DrawList`](crate::render::DrawList) from the finished layout.
    Render,
    /// End-of-frame bookkeeping (clears per-frame input edges).
    Cleanup,
}

pub trait Plugin {
    fn build(&self, app: &mut ZenithApp);
}

pub struct ZenithApp {
    pub world: World,
    started: bool,
}

impl ZenithApp {
    /// App with [`CorePlugin`] installed.
    pub fn new() -> Self {
        let mut app = Self::empty();
        app.add_plugin(CorePlugin);
        app
    }

    /// App with schedules only; no resources or systems.
    pub fn empty() -> Self {
        let mut world = World::new();
        world.add_schedule(Schedule::new(ZenithSchedule::Startup));
        let mut main = Schedule::new(ZenithSchedule::Main);
        main.configure_sets(
            (
                UiSet::Input,
                UiSet::Interaction,
                UiSet::Update,
                UiSet::Layout,
                UiSet::PreRender,
                UiSet::Render,
                UiSet::Cleanup,
            )
                .chain(),
        );
        world.add_schedule(main);
        Self {
            world,
            started: false,
        }
    }

    pub fn add_plugin(&mut self, plugin: impl Plugin) -> &mut Self {
        plugin.build(self);
        self
    }

    pub fn add_systems<M>(
        &mut self,
        schedule: ZenithSchedule,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.world.schedule_scope(schedule, |_, s| {
            s.add_systems(systems);
        });
        self
    }

    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn init_resource<R: Resource + FromWorld>(&mut self) -> &mut Self {
        self.world.init_resource::<R>();
        self
    }

    pub fn add_observer<E: EventPattern, M>(
        &mut self,
        observer: impl IntoObserverSystem<E, M>,
    ) -> &mut Self {
        self.world.add_observer(observer);
        self
    }

    /// Runs `Startup` (first call only), then one `Main` frame.
    pub fn update(&mut self) {
        if !self.started {
            self.started = true;
            self.world.run_schedule(ZenithSchedule::Startup);
        }
        self.world.run_schedule(ZenithSchedule::Main);
    }
}

impl Default for ZenithApp {
    fn default() -> Self {
        Self::new()
    }
}

/// Registers resources and the text, layout, interaction and draw-list systems.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut ZenithApp) {
        app.init_resource::<input::Viewport>()
            .init_resource::<input::PointerState>()
            .init_resource::<interaction::InteractionState>()
            .init_resource::<text::TextMeasurer>()
            .init_resource::<render::DrawList>()
            .add_systems(
                ZenithSchedule::Main,
                (
                    interaction::update_interaction.in_set(UiSet::Interaction),
                    (text::measure_text, widgets::button_visuals).in_set(UiSet::Update),
                    layout::compute_layout.in_set(UiSet::Layout),
                    render::extract_draw_list.in_set(UiSet::Render),
                    input::clear_pointer_edges.in_set(UiSet::Cleanup),
                ),
            );
    }
}
