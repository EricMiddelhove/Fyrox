use crate::{
    gui::UiNode,
    interaction::{
        move_mode::MoveInteractionMode, navmesh::EditNavmeshMode,
        rotate_mode::RotateInteractionMode, terrain::TerrainInteractionMode,
    },
    scene::{
        commands::{graph::ScaleNodeCommand, ChangeSelectionCommand, CommandGroup, SceneCommand},
        EditorScene, GraphSelection, Selection,
    },
    GameEngine, Message,
};
use rg3d::{
    core::{
        algebra::{Matrix4, UnitQuaternion, Vector2, Vector3},
        color::Color,
        math::{aabb::AxisAlignedBoundingBox, plane::Plane, Matrix4Ext},
        pool::Handle,
        scope_profile,
    },
    gui::message::{KeyCode, MessageDirection, WidgetMessage},
    scene::{
        base::BaseBuilder,
        graph::Graph,
        mesh::{
            surface::{SurfaceBuilder, SurfaceData},
            MeshBuilder, RenderPath,
        },
        node::Node,
        transform::TransformBuilder,
    },
};
use std::sync::{mpsc::Sender, Arc, RwLock};

pub mod move_mode;
pub mod navmesh;
pub mod rotate_mode;
pub mod terrain;

pub trait InteractionModeTrait {
    fn on_left_mouse_button_down(
        &mut self,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        mouse_pos: Vector2<f32>,
        frame_size: Vector2<f32>,
    );

    fn on_left_mouse_button_up(
        &mut self,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        mouse_pos: Vector2<f32>,
        frame_size: Vector2<f32>,
    );

    fn on_mouse_move(
        &mut self,
        mouse_offset: Vector2<f32>,
        mouse_position: Vector2<f32>,
        camera: Handle<Node>,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        frame_size: Vector2<f32>,
    );

    fn update(
        &mut self,
        editor_scene: &mut EditorScene,
        camera: Handle<Node>,
        engine: &mut GameEngine,
    );

    fn deactivate(&mut self, editor_scene: &EditorScene, engine: &mut GameEngine);

    fn on_key_down(
        &mut self,
        _key: KeyCode,
        _editor_scene: &mut EditorScene,
        _engine: &mut GameEngine,
    ) {
    }

    fn on_key_up(
        &mut self,
        _key: KeyCode,
        _editor_scene: &mut EditorScene,
        _engine: &mut GameEngine,
    ) {
    }
}

pub fn calculate_gizmo_distance_scaling(
    graph: &Graph,
    camera: Handle<Node>,
    gizmo_origin: Handle<Node>,
) -> Vector3<f32> {
    let distance = distance_scale_factor(graph[camera].as_camera().fov())
        * graph[gizmo_origin]
            .global_position()
            .metric_distance(&graph[camera].global_position());
    Vector3::new(distance, distance, distance)
}

fn distance_scale_factor(fov: f32) -> f32 {
    fov.tan() * 0.1
}

pub enum ScaleGizmoMode {
    None,
    X,
    Y,
    Z,
    Uniform,
}

pub struct ScaleGizmo {
    mode: ScaleGizmoMode,
    origin: Handle<Node>,
    x_arrow: Handle<Node>,
    y_arrow: Handle<Node>,
    z_arrow: Handle<Node>,
    x_axis: Handle<Node>,
    y_axis: Handle<Node>,
    z_axis: Handle<Node>,
}

fn make_scale_axis(
    graph: &mut Graph,
    rotation: UnitQuaternion<f32>,
    color: Color,
    name_prefix: &str,
) -> (Handle<Node>, Handle<Node>) {
    let arrow;
    let axis = MeshBuilder::new(
        BaseBuilder::new()
            .with_children(&[{
                arrow = MeshBuilder::new(
                    BaseBuilder::new()
                        .with_name(name_prefix.to_owned() + "Arrow")
                        .with_depth_offset(0.5)
                        .with_local_transform(
                            TransformBuilder::new()
                                .with_local_position(Vector3::new(0.0, 1.0, 0.0))
                                .build(),
                        ),
                )
                .with_render_path(RenderPath::Forward)
                .with_cast_shadows(false)
                .with_surfaces(vec![SurfaceBuilder::new(Arc::new(RwLock::new(
                    SurfaceData::make_cube(Matrix4::new_nonuniform_scaling(&Vector3::new(
                        0.1, 0.1, 0.1,
                    ))),
                )))
                .with_color(color)
                .build()])
                .build(graph);
                arrow
            }])
            .with_name(name_prefix.to_owned() + "Axis")
            .with_depth_offset(0.5)
            .with_local_transform(
                TransformBuilder::new()
                    .with_local_rotation(rotation)
                    .build(),
            ),
    )
    .with_render_path(RenderPath::Forward)
    .with_cast_shadows(false)
    .with_surfaces(vec![SurfaceBuilder::new(Arc::new(RwLock::new(
        SurfaceData::make_cylinder(10, 0.015, 1.0, true, &Matrix4::identity()),
    )))
    .with_color(color)
    .build()])
    .build(graph);

    (axis, arrow)
}

impl ScaleGizmo {
    pub fn new(editor_scene: &EditorScene, engine: &mut GameEngine) -> Self {
        let scene = &mut engine.scenes[editor_scene.scene];
        let graph = &mut scene.graph;

        let origin = MeshBuilder::new(
            BaseBuilder::new()
                .with_depth_offset(0.5)
                .with_name("Origin")
                .with_visibility(false),
        )
        .with_render_path(RenderPath::Forward)
        .with_cast_shadows(false)
        .with_surfaces(vec![SurfaceBuilder::new(Arc::new(RwLock::new(
            SurfaceData::make_cube(Matrix4::new_nonuniform_scaling(&Vector3::new(
                0.1, 0.1, 0.1,
            ))),
        )))
        .with_color(Color::opaque(0, 255, 255))
        .build()])
        .build(graph);

        graph.link_nodes(origin, editor_scene.root);

        let (x_axis, x_arrow) = make_scale_axis(
            graph,
            UnitQuaternion::from_axis_angle(&Vector3::z_axis(), 90.0f32.to_radians()),
            Color::RED,
            "X",
        );
        graph.link_nodes(x_axis, origin);
        let (y_axis, y_arrow) = make_scale_axis(
            graph,
            UnitQuaternion::from_axis_angle(&Vector3::z_axis(), 0.0f32.to_radians()),
            Color::GREEN,
            "Y",
        );
        graph.link_nodes(y_axis, origin);
        let (z_axis, z_arrow) = make_scale_axis(
            graph,
            UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 90.0f32.to_radians()),
            Color::BLUE,
            "Z",
        );
        graph.link_nodes(z_axis, origin);

        Self {
            mode: ScaleGizmoMode::None,
            origin,
            x_arrow,
            y_arrow,
            z_arrow,
            x_axis,
            y_axis,
            z_axis,
        }
    }

    pub fn set_mode(&mut self, mode: ScaleGizmoMode, graph: &mut Graph) {
        self.mode = mode;

        // Restore initial colors first.
        graph[self.origin]
            .as_mesh_mut()
            .set_color(Color::opaque(0, 255, 255));
        graph[self.x_axis].as_mesh_mut().set_color(Color::RED);
        graph[self.x_arrow].as_mesh_mut().set_color(Color::RED);
        graph[self.y_axis].as_mesh_mut().set_color(Color::GREEN);
        graph[self.y_arrow].as_mesh_mut().set_color(Color::GREEN);
        graph[self.z_axis].as_mesh_mut().set_color(Color::BLUE);
        graph[self.z_arrow].as_mesh_mut().set_color(Color::BLUE);

        let yellow = Color::opaque(255, 255, 0);
        match self.mode {
            ScaleGizmoMode::None => (),
            ScaleGizmoMode::X => {
                graph[self.x_axis].as_mesh_mut().set_color(yellow);
                graph[self.x_arrow].as_mesh_mut().set_color(yellow);
            }
            ScaleGizmoMode::Y => {
                graph[self.y_axis].as_mesh_mut().set_color(yellow);
                graph[self.y_arrow].as_mesh_mut().set_color(yellow);
            }
            ScaleGizmoMode::Z => {
                graph[self.z_axis].as_mesh_mut().set_color(yellow);
                graph[self.z_arrow].as_mesh_mut().set_color(yellow);
            }
            ScaleGizmoMode::Uniform => {
                graph[self.origin].as_mesh_mut().set_color(yellow);
            }
        }
    }

    pub fn handle_pick(
        &mut self,
        picked: Handle<Node>,
        editor_scene: &EditorScene,
        engine: &mut GameEngine,
    ) -> bool {
        let graph = &mut engine.scenes[editor_scene.scene].graph;

        if picked == self.x_axis || picked == self.x_arrow {
            self.set_mode(ScaleGizmoMode::X, graph);
            true
        } else if picked == self.y_axis || picked == self.y_arrow {
            self.set_mode(ScaleGizmoMode::Y, graph);
            true
        } else if picked == self.z_axis || picked == self.z_arrow {
            self.set_mode(ScaleGizmoMode::Z, graph);
            true
        } else if picked == self.origin {
            self.set_mode(ScaleGizmoMode::Uniform, graph);
            true
        } else {
            self.set_mode(ScaleGizmoMode::None, graph);
            false
        }
    }

    pub fn calculate_scale_delta(
        &self,
        editor_scene: &EditorScene,
        camera: Handle<Node>,
        mouse_offset: Vector2<f32>,
        mouse_position: Vector2<f32>,
        engine: &GameEngine,
        frame_size: Vector2<f32>,
    ) -> Vector3<f32> {
        let graph = &engine.scenes[editor_scene.scene].graph;
        let node_global_transform = graph[self.origin].global_transform();

        if let Node::Camera(camera) = &graph[camera] {
            let inv_node_transform = node_global_transform.try_inverse().unwrap_or_default();

            // Create two rays in object space.
            let initial_ray = camera
                .make_ray(mouse_position, frame_size)
                .transform(inv_node_transform);
            let offset_ray = camera
                .make_ray(mouse_position + mouse_offset, frame_size)
                .transform(inv_node_transform);

            let dlook = inv_node_transform
                .transform_vector(&(node_global_transform.position() - camera.global_position()));

            // Select plane by current active mode.
            let plane = match self.mode {
                ScaleGizmoMode::None => return Vector3::default(),
                ScaleGizmoMode::X => Plane::from_normal_and_point(
                    &Vector3::new(0.0, dlook.y, dlook.z),
                    &Vector3::default(),
                ),
                ScaleGizmoMode::Y => Plane::from_normal_and_point(
                    &Vector3::new(dlook.x, 0.0, dlook.z),
                    &Vector3::default(),
                ),
                ScaleGizmoMode::Z => Plane::from_normal_and_point(
                    &Vector3::new(dlook.x, dlook.y, 0.0),
                    &Vector3::default(),
                ),
                ScaleGizmoMode::Uniform => {
                    Plane::from_normal_and_point(&dlook, &Vector3::default())
                }
            }
            .unwrap_or_default();

            // Get two intersection points with plane and use delta between them to calculate scale delta.
            if let Some(initial_point) = initial_ray.plane_intersection_point(&plane) {
                if let Some(next_point) = offset_ray.plane_intersection_point(&plane) {
                    let delta = next_point - initial_point;
                    return match self.mode {
                        ScaleGizmoMode::None => unreachable!(),
                        ScaleGizmoMode::X => Vector3::new(-delta.x, 0.0, 0.0),
                        ScaleGizmoMode::Y => Vector3::new(0.0, delta.y, 0.0),
                        ScaleGizmoMode::Z => Vector3::new(0.0, 0.0, delta.z),
                        ScaleGizmoMode::Uniform => {
                            // TODO: Still may behave weird.
                            let amount = delta.norm() * (delta.y + delta.x + delta.z).signum();
                            Vector3::new(amount, amount, amount)
                        }
                    };
                }
            }
        }

        Vector3::default()
    }

    pub fn sync_transform(
        &self,
        graph: &mut Graph,
        selection: &GraphSelection,
        scale: Vector3<f32>,
    ) {
        if let Some((rotation, position)) = selection.global_rotation_position(graph) {
            graph[self.origin]
                .set_visibility(true)
                .local_transform_mut()
                .set_rotation(rotation)
                .set_position(position)
                .set_scale(scale);
        }
    }

    pub fn set_visible(&self, graph: &mut Graph, visible: bool) {
        graph[self.origin].set_visibility(visible);
    }
}

pub struct ScaleInteractionMode {
    initial_scales: Vec<Vector3<f32>>,
    scale_gizmo: ScaleGizmo,
    interacting: bool,
    message_sender: Sender<Message>,
}

impl ScaleInteractionMode {
    pub fn new(
        editor_scene: &EditorScene,
        engine: &mut GameEngine,
        message_sender: Sender<Message>,
    ) -> Self {
        Self {
            initial_scales: Default::default(),
            scale_gizmo: ScaleGizmo::new(editor_scene, engine),
            interacting: false,
            message_sender,
        }
    }
}

impl InteractionModeTrait for ScaleInteractionMode {
    fn on_left_mouse_button_down(
        &mut self,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        mouse_pos: Vector2<f32>,
        frame_size: Vector2<f32>,
    ) {
        if let Selection::Graph(selection) = &editor_scene.selection {
            let graph = &mut engine.scenes[editor_scene.scene].graph;

            // Pick gizmo nodes.
            let camera = editor_scene.camera_controller.camera;
            let camera_pivot = editor_scene.camera_controller.pivot;
            let editor_node = editor_scene.camera_controller.pick(
                mouse_pos,
                graph,
                editor_scene.root,
                frame_size,
                true,
                |handle, _| handle != camera && handle != camera_pivot,
            );

            if self
                .scale_gizmo
                .handle_pick(editor_node, editor_scene, engine)
            {
                let graph = &mut engine.scenes[editor_scene.scene].graph;
                self.interacting = true;
                self.initial_scales = selection.local_scales(graph);
            }
        }
    }

    fn on_left_mouse_button_up(
        &mut self,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        mouse_pos: Vector2<f32>,
        frame_size: Vector2<f32>,
    ) {
        let graph = &mut engine.scenes[editor_scene.scene].graph;

        if self.interacting {
            if let Selection::Graph(selection) = &editor_scene.selection {
                if !selection.is_empty() {
                    self.interacting = false;
                    let current_scales = selection.local_scales(graph);
                    if current_scales != self.initial_scales {
                        // Commit changes.
                        let commands = CommandGroup::from(
                            selection
                                .nodes()
                                .iter()
                                .zip(self.initial_scales.iter().zip(current_scales.iter()))
                                .map(|(&node, (&old_scale, &new_scale))| {
                                    SceneCommand::ScaleNode(ScaleNodeCommand::new(
                                        node, old_scale, new_scale,
                                    ))
                                })
                                .collect::<Vec<SceneCommand>>(),
                        );
                        self.message_sender
                            .send(Message::DoSceneCommand(SceneCommand::CommandGroup(
                                commands,
                            )))
                            .unwrap();
                    }
                }
            }
        } else {
            let picked = editor_scene.camera_controller.pick(
                mouse_pos,
                graph,
                editor_scene.root,
                frame_size,
                false,
                |_, _| true,
            );
            let new_selection =
                if engine.user_interface.keyboard_modifiers().control && picked.is_some() {
                    if let Selection::Graph(selection) = &editor_scene.selection {
                        let mut selection = selection.clone();
                        selection.insert_or_exclude(picked);
                        Selection::Graph(selection)
                    } else {
                        Selection::Graph(GraphSelection::single_or_empty(picked))
                    }
                } else {
                    Selection::Graph(GraphSelection::single_or_empty(picked))
                };
            if new_selection != editor_scene.selection {
                self.message_sender
                    .send(Message::DoSceneCommand(SceneCommand::ChangeSelection(
                        ChangeSelectionCommand::new(new_selection, editor_scene.selection.clone()),
                    )))
                    .unwrap();
            }
        }
    }

    fn on_mouse_move(
        &mut self,
        mouse_offset: Vector2<f32>,
        mouse_position: Vector2<f32>,
        camera: Handle<Node>,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        frame_size: Vector2<f32>,
    ) {
        if let Selection::Graph(selection) = &editor_scene.selection {
            if self.interacting {
                let scale_delta = self.scale_gizmo.calculate_scale_delta(
                    editor_scene,
                    camera,
                    mouse_offset,
                    mouse_position,
                    engine,
                    frame_size,
                );
                for &node in selection.nodes().iter() {
                    let transform =
                        engine.scenes[editor_scene.scene].graph[node].local_transform_mut();
                    let initial_scale = transform.scale();
                    let sx = (initial_scale.x * (1.0 + scale_delta.x)).max(std::f32::EPSILON);
                    let sy = (initial_scale.y * (1.0 + scale_delta.y)).max(std::f32::EPSILON);
                    let sz = (initial_scale.z * (1.0 + scale_delta.z)).max(std::f32::EPSILON);
                    transform.set_scale(Vector3::new(sx, sy, sz));
                }
            }
        }
    }

    fn update(
        &mut self,
        editor_scene: &mut EditorScene,
        camera: Handle<Node>,
        engine: &mut GameEngine,
    ) {
        if let Selection::Graph(selection) = &editor_scene.selection {
            if !editor_scene.selection.is_empty() {
                let graph = &mut engine.scenes[editor_scene.scene].graph;
                let scale =
                    calculate_gizmo_distance_scaling(graph, camera, self.scale_gizmo.origin);
                self.scale_gizmo.sync_transform(graph, selection, scale);
                self.scale_gizmo.set_visible(graph, true);
            } else {
                let graph = &mut engine.scenes[editor_scene.scene].graph;
                self.scale_gizmo.set_visible(graph, false);
            }
        }
    }

    fn deactivate(&mut self, editor_scene: &EditorScene, engine: &mut GameEngine) {
        let graph = &mut engine.scenes[editor_scene.scene].graph;
        self.scale_gizmo.set_visible(graph, false);
    }
}

pub struct SelectInteractionMode {
    preview: Handle<UiNode>,
    selection_frame: Handle<UiNode>,
    message_sender: Sender<Message>,
    stack: Vec<Handle<Node>>,
    click_pos: Vector2<f32>,
}

impl SelectInteractionMode {
    pub fn new(
        preview: Handle<UiNode>,
        selection_frame: Handle<UiNode>,
        message_sender: Sender<Message>,
    ) -> Self {
        Self {
            preview,
            selection_frame,
            message_sender,
            stack: Vec::new(),
            click_pos: Vector2::default(),
        }
    }
}

impl InteractionModeTrait for SelectInteractionMode {
    fn on_left_mouse_button_down(
        &mut self,
        _editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        mouse_pos: Vector2<f32>,
        _frame_size: Vector2<f32>,
    ) {
        self.click_pos = mouse_pos;
        let ui = &mut engine.user_interface;
        ui.send_message(WidgetMessage::visibility(
            self.selection_frame,
            MessageDirection::ToWidget,
            true,
        ));
        ui.send_message(WidgetMessage::desired_position(
            self.selection_frame,
            MessageDirection::ToWidget,
            mouse_pos,
        ));
        ui.send_message(WidgetMessage::width(
            self.selection_frame,
            MessageDirection::ToWidget,
            0.0,
        ));
        ui.send_message(WidgetMessage::height(
            self.selection_frame,
            MessageDirection::ToWidget,
            0.0,
        ));
    }

    fn on_left_mouse_button_up(
        &mut self,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        _mouse_pos: Vector2<f32>,
        frame_size: Vector2<f32>,
    ) {
        let scene = &engine.scenes[editor_scene.scene];
        let camera = scene.graph[editor_scene.camera_controller.camera].as_camera();
        let preview_screen_bounds = engine.user_interface.node(self.preview).screen_bounds();
        let frame_screen_bounds = engine
            .user_interface
            .node(self.selection_frame)
            .screen_bounds();
        let relative_bounds = frame_screen_bounds.translate(-preview_screen_bounds.position);
        self.stack.clear();
        self.stack.push(scene.graph.get_root());
        let mut graph_selection = GraphSelection::default();
        while let Some(handle) = self.stack.pop() {
            let node = &scene.graph[handle];
            if handle == editor_scene.root {
                continue;
            }
            if handle == scene.graph.get_root() {
                self.stack.extend_from_slice(node.children());
                continue;
            }
            let aabb = match node {
                Node::Base(_) => AxisAlignedBoundingBox::unit(),
                Node::Light(_) => AxisAlignedBoundingBox::unit(),
                Node::Camera(_) => AxisAlignedBoundingBox::unit(),
                Node::Mesh(mesh) => mesh.bounding_box(),
                Node::Sprite(_) => AxisAlignedBoundingBox::unit(),
                Node::ParticleSystem(_) => AxisAlignedBoundingBox::unit(),
                Node::Terrain(ref terrain) => terrain.bounding_box(),
            };

            for screen_corner in aabb
                .corners()
                .iter()
                .filter_map(|&p| camera.project(p + node.global_position(), frame_size))
            {
                if relative_bounds.contains(screen_corner) {
                    graph_selection.insert_or_exclude(handle);
                    break;
                }
            }

            self.stack.extend_from_slice(node.children());
        }

        let new_selection = Selection::Graph(graph_selection);

        if !new_selection.is_empty() && new_selection != editor_scene.selection {
            self.message_sender
                .send(Message::DoSceneCommand(SceneCommand::ChangeSelection(
                    ChangeSelectionCommand::new(new_selection, editor_scene.selection.clone()),
                )))
                .unwrap();
        }
        engine
            .user_interface
            .send_message(WidgetMessage::visibility(
                self.selection_frame,
                MessageDirection::ToWidget,
                false,
            ));
    }

    fn on_mouse_move(
        &mut self,
        _mouse_offset: Vector2<f32>,
        mouse_position: Vector2<f32>,
        _camera: Handle<Node>,
        _editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        _frame_size: Vector2<f32>,
    ) {
        let ui = &mut engine.user_interface;
        let width = mouse_position.x - self.click_pos.x;
        let height = mouse_position.y - self.click_pos.y;

        let position = Vector2::new(
            if width < 0.0 {
                mouse_position.x
            } else {
                self.click_pos.x
            },
            if height < 0.0 {
                mouse_position.y
            } else {
                self.click_pos.y
            },
        );
        ui.send_message(WidgetMessage::desired_position(
            self.selection_frame,
            MessageDirection::ToWidget,
            position,
        ));
        ui.send_message(WidgetMessage::width(
            self.selection_frame,
            MessageDirection::ToWidget,
            width.abs(),
        ));
        ui.send_message(WidgetMessage::height(
            self.selection_frame,
            MessageDirection::ToWidget,
            height.abs(),
        ));
    }

    fn update(
        &mut self,
        _editor_scene: &mut EditorScene,
        _camera: Handle<Node>,
        _engine: &mut GameEngine,
    ) {
    }

    fn deactivate(&mut self, _editor_scene: &EditorScene, _engine: &mut GameEngine) {}
}

/// Helper enum to be able to access interaction modes in array directly.
#[derive(Copy, Clone, PartialOrd, PartialEq, Hash, Debug)]
#[repr(usize)]
pub enum InteractionModeKind {
    Select = 0,
    Move = 1,
    Scale = 2,
    Rotate = 3,
    Navmesh = 4,
    Terrain = 5,
}

pub enum InteractionMode {
    Select(SelectInteractionMode),
    Move(MoveInteractionMode),
    Scale(ScaleInteractionMode),
    Rotate(RotateInteractionMode),
    Navmesh(EditNavmeshMode),
    Terrain(TerrainInteractionMode),
}

macro_rules! static_dispatch {
    ($self:ident, $func:ident, $($args:expr),*) => {
        match $self {
            InteractionMode::Select(v) => v.$func($($args),*),
            InteractionMode::Move(v) => v.$func($($args),*),
            InteractionMode::Scale(v) => v.$func($($args),*),
            InteractionMode::Rotate(v) => v.$func($($args),*),
            InteractionMode::Navmesh(v) => v.$func($($args),*),
            InteractionMode::Terrain(v) => v.$func($($args),*),
        }
    }
}

impl InteractionModeTrait for InteractionMode {
    fn on_left_mouse_button_down(
        &mut self,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        mouse_pos: Vector2<f32>,
        frame_size: Vector2<f32>,
    ) {
        scope_profile!();

        static_dispatch!(
            self,
            on_left_mouse_button_down,
            editor_scene,
            engine,
            mouse_pos,
            frame_size
        )
    }

    fn on_left_mouse_button_up(
        &mut self,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        mouse_pos: Vector2<f32>,
        frame_size: Vector2<f32>,
    ) {
        scope_profile!();

        static_dispatch!(
            self,
            on_left_mouse_button_up,
            editor_scene,
            engine,
            mouse_pos,
            frame_size
        )
    }

    fn on_mouse_move(
        &mut self,
        mouse_offset: Vector2<f32>,
        mouse_position: Vector2<f32>,
        camera: Handle<Node>,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
        frame_size: Vector2<f32>,
    ) {
        scope_profile!();

        static_dispatch!(
            self,
            on_mouse_move,
            mouse_offset,
            mouse_position,
            camera,
            editor_scene,
            engine,
            frame_size
        )
    }

    fn update(
        &mut self,
        editor_scene: &mut EditorScene,
        camera: Handle<Node>,
        engine: &mut GameEngine,
    ) {
        scope_profile!();

        static_dispatch!(self, update, editor_scene, camera, engine)
    }

    fn deactivate(&mut self, editor_scene: &EditorScene, engine: &mut GameEngine) {
        scope_profile!();

        static_dispatch!(self, deactivate, editor_scene, engine)
    }

    fn on_key_down(
        &mut self,
        key: KeyCode,
        editor_scene: &mut EditorScene,
        engine: &mut GameEngine,
    ) {
        scope_profile!();

        static_dispatch!(self, on_key_down, key, editor_scene, engine)
    }

    fn on_key_up(&mut self, key: KeyCode, editor_scene: &mut EditorScene, engine: &mut GameEngine) {
        scope_profile!();

        static_dispatch!(self, on_key_up, key, editor_scene, engine)
    }
}
