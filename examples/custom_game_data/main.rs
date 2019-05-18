//! Demonstrates how to use a custom game data structure

use crate::{
    example_system::ExampleSystem,
    game_data::{CustomGameData, CustomGameDataBuilder},
};
use amethyst::{
    assets::{Completion, ProgressCounter, Prefab, PrefabLoader, PrefabLoaderSystem, Handle, RonFormat},
    core::{frame_limiter::FrameRateLimitStrategy, Transform, transform::TransformBundle, Time, math::Vector3, math::Point3,},
    ecs::prelude::{Component, Entity, ReadExpect, Resources, System, SystemData, Write},
    ecs::NullStorage,
    input::{is_close_requested, is_key_down, InputBundle, StringBindings},
    prelude::*,
    renderer::{
        Mesh,
        debug_drawing::{DebugLines, DebugLinesComponent, DebugLinesParams},
        palette::{Srgb, Srgba},
        rendy::{
            factory::Factory,
            graph::{
                render::{RenderGroupDesc, SubpassBuilder},
                GraphBuilder,
            },
            hal::format::Format,
            mesh::{Normal, Position, TexCoord},
        },
        pass::DrawShadedDesc, pass::DrawDebugLinesDesc,
        types::DefaultBackend,
        GraphCreator, RenderingSystem,
    },
    shrev::{EventChannel, ReaderId},
    ui::{DrawUiDesc, UiBundle, UiCreator, UiEvent, UiFinder, UiText, UiLoader, UiPrefab},
    utils::{
        application_root_dir,
        fps_counter::{FPSCounter, FPSCounterBundle},
        scene::BasicScenePrefab,
    },
    window::{ScreenDimensions, Window, WindowBundle},
    winit::VirtualKeyCode,
    Error,
};

mod example_system;
mod game_data;

type MyPrefabData = BasicScenePrefab<(Vec<Position>, Vec<Normal>, Vec<TexCoord>)>;

pub struct DemoState {
    light_angle: f32,
    light_color: Srgb,
    camera_angle: f32,
}

#[derive(Default)]
struct Loading {
    progress: ProgressCounter,
    scene: Option<Handle<Prefab<MyPrefabData>>>,
    load_ui: Option<Entity>,
    paused_ui: Option<Handle<UiPrefab>>,
}
struct Main {
    scene: Handle<Prefab<MyPrefabData>>,
    paused_ui: Handle<UiPrefab>,
}
struct Paused {
    ui: Entity,
}

#[derive(Default)]
struct Tag;

impl Component for Tag {
    type Storage = NullStorage<Self>;
}

impl<'a, 'b> State<CustomGameData<'a, 'b>, StateEvent> for Loading {
    fn on_start(&mut self, data: StateData<'_, CustomGameData<'_, '_>>) {
        self.scene = Some(data.world.exec(|loader: PrefabLoader<'_, MyPrefabData>| {
            loader.load("prefab/renderable.ron", RonFormat, &mut self.progress)
        }));
       
        self.load_ui = Some(data.world.exec(|mut creator: UiCreator<'_>| {
            creator.create("ui/fps.ron", &mut self.progress);
            creator.create("ui/loading.ron", &mut self.progress)
        }));
        self.paused_ui = Some(
            data.world
                .exec(|loader: UiLoader<'_>| loader.load("ui/paused.ron", &mut self.progress)),
        );
        data.world.add_resource::<DemoState>(DemoState {
            light_angle: 0.0,
            light_color: Srgb::new(1.0, 1.0, 1.0),
            camera_angle: 0.0,
        });
    }

    fn handle_event(
        &mut self,
        _: StateData<'_, CustomGameData<'_, '_>>,
        event: StateEvent,
    ) -> Trans<CustomGameData<'a, 'b>, StateEvent> {
        if let StateEvent::Window(event) = event {
            if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
                return Trans::Quit;
            }
        }
        Trans::None
    }

    fn update(
        &mut self,
        data: StateData<'_, CustomGameData<'_, '_>>,
    ) -> Trans<CustomGameData<'a, 'b>, StateEvent> {
        data.data.update(&data.world, true);
        match self.progress.complete() {
            Completion::Failed => {
                eprintln!("Failed loading assets");
                Trans::Quit
            }
            Completion::Complete => {
                println!("Assets loaded, swapping state");
                if let Some(entity) = self.load_ui {
                    let _ = data.world.delete_entity(entity);
                }
                Trans::Switch(Box::new(Main {
                    scene: self.scene.as_ref().unwrap().clone(),
                    paused_ui: self.paused_ui.as_ref().unwrap().clone(),
                }))
            }
            Completion::Loading => Trans::None,
        }
    }
}

impl<'a, 'b> State<CustomGameData<'a, 'b>, StateEvent> for Paused {
    fn handle_event(
        &mut self,
        data: StateData<'_, CustomGameData<'_, '_>>,
        event: StateEvent,
    ) -> Trans<CustomGameData<'a, 'b>, StateEvent> {
        if let StateEvent::Window(event) = &event {
            if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
                Trans::Quit
            } else if is_key_down(&event, VirtualKeyCode::Space) {
                let _ = data.world.delete_entity(self.ui);
                Trans::Pop
            } else {
                Trans::None
            }
        } else {
            Trans::None
        }
    }

    fn update(
        &mut self,
        data: StateData<'_, CustomGameData<'_, '_>>,
    ) -> Trans<CustomGameData<'a, 'b>, StateEvent> {
        data.data.update(&data.world, false);
        Trans::None
    }
}

impl<'a, 'b> State<CustomGameData<'a, 'b>, StateEvent> for Main {
    fn on_start(&mut self, data: StateData<'_, CustomGameData<'_, '_>>) {
        data.world.create_entity().with(self.scene.clone()).build();


        // Setup debug lines as a component and add lines to render axis&grid
        let mut debug_lines_component = DebugLinesComponent::with_capacity(100);
        debug_lines_component.add_direction(
            [0.0, 0.0001, 0.0].into(),
            [0.2, 0.0, 0.0].into(),
            Srgba::new(1.0, 0.0, 0.23, 1.0),
        );
        debug_lines_component.add_direction(
            [0.0, 0.0, 0.0].into(),
            [0.0, 0.2, 0.0].into(),
            Srgba::new(0.5, 0.85, 0.1, 1.0),
        );
        debug_lines_component.add_direction(
            [0.0, 0.0001, 0.0].into(),
            [0.0, 0.0, 0.2].into(),
            Srgba::new(0.2, 0.75, 0.93, 1.0),
        );

        let width: u32 = 50;
        let depth: u32 = 50;
        let main_color = Srgba::new(0.4, 0.4, 0.4, 1.0);

        // Grid lines in X-axis
        for x in 0..=width {
            let (x, width, depth) = (x as f32, width as f32, depth as f32);

            let position = Point3::new(x - width / 2.0, 0.0, -depth / 2.0);
            let direction = Vector3::new(0.0, 0.0, depth);

            debug_lines_component.add_direction(position, direction, main_color);

            // Sub-grid lines
            if x != width {
                for sub_x in 1..10 {
                    let sub_offset = Vector3::new((1.0 / 10.0) * sub_x as f32, -0.001, 0.0);

                    debug_lines_component.add_direction(
                        position + sub_offset,
                        direction,
                        Srgba::new(0.1, 0.1, 0.1, 0.1),
                    );
                }
            }
        }

        // Grid lines in Z-axis
        for z in 0..=depth {
            let (z, width, depth) = (z as f32, width as f32, depth as f32);

            let position = Point3::new(-width / 2.0, 0.0, z - depth / 2.0);
            let direction = Vector3::new(width, 0.0, 0.0);

            debug_lines_component.add_direction(position, direction, main_color);

            // Sub-grid lines
            if z != depth {
                for sub_z in 1..10 {
                    let sub_offset = Vector3::new(0.0, -0.001, (1.0 / 10.0) * sub_z as f32);

                    debug_lines_component.add_direction(
                        position + sub_offset,
                        direction,
                        Srgba::new(0.1, 0.1, 0.1, 0.0),
                    );
                }
            }
        }
        data.world.register::<DebugLinesComponent>();
        data.world
            .create_entity()
            .with(debug_lines_component)
            .build();


    }

    fn handle_event(
        &mut self,
        data: StateData<'_, CustomGameData<'_, '_>>,
        event: StateEvent,
    ) -> Trans<CustomGameData<'a, 'b>, StateEvent> {
        if let StateEvent::Window(event) = &event {
            if is_close_requested(&event) || is_key_down(&event, VirtualKeyCode::Escape) {
                Trans::Quit
            } else if is_key_down(&event, VirtualKeyCode::Space) {
                Trans::Push(Box::new(Paused {
                    ui: data
                        .world
                        .create_entity()
                        .with(self.paused_ui.clone())
                        .build(),
                }))
            } else {
                Trans::None
            }
        } else {
            Trans::None
        }
    }

    fn update(
        &mut self,
        data: StateData<'_, CustomGameData<'_, '_>>,
    ) -> Trans<CustomGameData<'a, 'b>, StateEvent> {
        data.data.update(&data.world, true);
        Trans::None
    }
}

fn main() -> Result<(), Error> {
    amethyst::Logger::from_config(amethyst::LoggerConfig {
        level_filter: log::LevelFilter::Info,
        ..Default::default()
    })
    .level_for("custom_game_data", log::LevelFilter::Debug)
    .start();

    let app_root = application_root_dir()?;

    // Add our meshes directory to the asset loader.
    let asset_dir = app_root.join("examples/assets");

    let display_config_path =
        app_root.join("examples/custom_game_data/resources/display_config.ron");

    // let pipeline_builder = Pipeline::build().with_stage(
    //     Stage::with_backbuffer()
    //         .clear_target([0.0, 0.0, 0.0, 1.0], 1.0)
    //         .with_pass(DrawShaded::<PosNormTex>::new())
    //         .with_pass(DrawUi::new()),
    // );
    let game_data = CustomGameDataBuilder::default()
        .with_base(PrefabLoaderSystem::<MyPrefabData>::default(), "", &[])
        .with_running::<ExampleSystem>(ExampleSystem::default(), "example_system", &[])
        .with_base_bundle(TransformBundle::new())?
        .with_base_bundle(UiBundle::<DefaultBackend, StringBindings>::new())?
        .with_base_bundle(FPSCounterBundle::default())?
        .with_base_bundle(InputBundle::<StringBindings>::new())?
        .with_base_bundle(WindowBundle::from_config_path(display_config_path))?
        .with_thread_local(RenderingSystem::<DefaultBackend, _>::new(
            ExampleGraph::default(),
        ));

    let mut game = Application::build(asset_dir, Loading::default())?.build(game_data)?;
    game.run();

    Ok(())
}



#[derive(Default)]
struct ExampleGraph {
    last_dimensions: Option<ScreenDimensions>,
    surface_format: Option<Format>,
    dirty: bool,
}

impl GraphCreator<DefaultBackend> for ExampleGraph {
    fn rebuild(&mut self, res: &Resources) -> bool {
        // Rebuild when dimensions change, but wait until at least two frames have the same.
        let new_dimensions = res.try_fetch::<ScreenDimensions>();
        use std::ops::Deref;
        if self.last_dimensions.as_ref() != new_dimensions.as_ref().map(|d| d.deref()) {
            self.dirty = true;
            self.last_dimensions = new_dimensions.map(|d| d.clone());
            return false;
        }
        return self.dirty;
    }

    fn builder(
        &mut self,
        factory: &mut Factory<DefaultBackend>,
        res: &Resources,
    ) -> GraphBuilder<DefaultBackend, Resources> {
        use amethyst::renderer::rendy::{
            graph::present::PresentNode,
            hal::command::{ClearDepthStencil, ClearValue},
        };

        self.dirty = false;
        let window = <ReadExpect<'_, std::sync::Arc<Window>>>::fetch(res);
        let surface = factory.create_surface(window.clone());
        // cache surface format to speed things up
        let surface_format = *self
            .surface_format
            .get_or_insert_with(|| factory.get_surface_format(&surface));

        let mut graph_builder = GraphBuilder::new();
        let color = graph_builder.create_image(
            surface.kind(),
            1,
            surface_format,
            Some(ClearValue::Color([0.0, 0.0, 0.0, 1.0].into())),
        );

        let depth = graph_builder.create_image(
            surface.kind(),
            1,
            Format::D32Float,
            Some(ClearValue::DepthStencil(ClearDepthStencil(1.0, 0))),
        );
        let opaque = graph_builder.add_node(
            SubpassBuilder::new()
                .with_group(DrawDebugLinesDesc::default().builder())
                .with_group(DrawShadedDesc::default().builder())
                .with_color(color)
                .with_depth_stencil(depth)
                .into_pass(),
        );

        let ui = graph_builder.add_node(
            SubpassBuilder::new()
                .with_group(DrawUiDesc::default().builder().with_dependency(opaque))
                .with_color(color)
                .with_depth_stencil(depth)
                .into_pass(),
        );

        let _present = graph_builder
            .add_node(PresentNode::builder(factory, surface, color).with_dependency(ui));

        graph_builder
    }
}
