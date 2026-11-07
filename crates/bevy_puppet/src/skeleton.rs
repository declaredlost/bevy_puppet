//! Puppets in the world: a skeleton of bone entities, built from each puppet's
//! rig, posed and placed every frame.

use std::collections::{HashMap, HashSet};
use std::f32::consts::FRAC_PI_2;

use bevy::asset::{AssetEvent, AssetId, Assets, Handle};
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

use crate::clip::Clip;
use crate::label::RoleLabel;
use crate::pose::{Animator, Gait, Goals, Layers, Params, Placement, Pose, Side};
use crate::rig::Rig;

/// How far apart bones of neighbouring depths are drawn, in z.
const DEPTH: f32 = 0.01;

/// How near a whole number of quarter turns an angle is taken for one, in
/// radians: turned exactly, with no hair off.
const QUARTER: f32 = 1e-3;

/// A puppet: the rig its skeleton of bone entities is built from.
///
/// Once the rig loads, it gets a [`Skeleton`] child, and under it an entity for
/// each bone ([`Bones`]), posed by its [`Animator`] and placed every frame.
/// What it shows is whatever the game parents under its bones: they move, turn
/// and mirror with them.
#[derive(Component, Debug, Clone)]
#[require(Animator, Pose, Side, Params, Goals, Gait, Transform)]
#[cfg_attr(feature = "render", require(Visibility))]
pub struct Puppet(pub Handle<Rig>);

/// A puppet's skeleton: the root its bones hang from, mirrored as it faces.
///
/// Mirrored exactly (its `scale.x`) as the puppet faces left.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[require(Transform)]
#[cfg_attr(feature = "render", require(Visibility))]
pub struct Skeleton {
    puppet: Entity,
}

/// A bone of a puppet: an entity under its parent bone, posed every frame.
///
/// By its index in the puppet's rig: its transform is its joint, every frame,
/// under its parent bone's.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[require(Placement, Transform)]
#[cfg_attr(feature = "render", require(Visibility))]
pub struct Bone {
    puppet: Entity,
    index: usize,
}

/// A puppet's bone entities, by the bone's index in its rig, and the skeleton
/// they hang from.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Bones {
    root: Entity,
    entities: Vec<Entity>,
    rig: AssetId<Rig>,
}

impl Skeleton {
    /// The puppet it's the skeleton of.
    #[must_use]
    pub fn puppet(&self) -> Entity {
        self.puppet
    }
}

impl Bone {
    /// The puppet it's a bone of.
    #[must_use]
    pub fn puppet(&self) -> Entity {
        self.puppet
    }

    /// Its index in the puppet's rig.
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }
}

impl Bones {
    /// The skeleton's root.
    #[must_use]
    pub fn root(&self) -> Entity {
        self.root
    }

    /// The entity of bone `index`.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Entity> {
        self.entities.get(index).copied()
    }

    /// The entity of the bone named `name` on `rig`, the puppet's.
    #[must_use]
    pub fn named(&self, rig: &Rig, name: &str) -> Option<Entity> {
        rig.index(name).and_then(|index| self.get(index))
    }

    /// The entity of the bone doing `role` on `rig`, the puppet's.
    #[must_use]
    pub fn role(&self, rig: &Rig, role: impl RoleLabel) -> Option<Entity> {
        rig.role(role).and_then(|index| self.get(index))
    }

    /// Every bone's entity, by index.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entities.iter().copied()
    }
}

/// Builds each puppet's skeleton once its rig loads, and again as its rig
/// changes: a bone that keeps its name keeps its entity, and what the game put
/// under it.
pub(crate) fn build(
    mut commands: Commands,
    rigs: Res<Assets<Rig>>,
    mut changed: MessageReader<AssetEvent<Rig>>,
    puppets: Query<(Entity, &Puppet, Option<&Bones>)>,
    names: Query<&Name>,
) {
    let modified: HashSet<AssetId<Rig>> = changed
        .read()
        .filter_map(|event| match event {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    for (puppet, Puppet(handle), had) in &puppets {
        let id = handle.id();
        let Some(rig) = rigs.get(id) else {
            continue;
        };
        if had.is_some_and(|bones| bones.rig == id && !modified.contains(&id)) {
            continue;
        }
        let root = had.map_or_else(
            || {
                commands
                    .spawn((Name::new("Skeleton"), Skeleton { puppet }, ChildOf(puppet)))
                    .id()
            },
            |bones| bones.root,
        );
        let mut old: HashMap<String, Entity> = had
            .into_iter()
            .flat_map(Bones::iter)
            .filter_map(|entity| Some((names.get(entity).ok()?.as_str().to_owned(), entity)))
            .collect();
        let mut entities: Vec<Entity> = Vec::with_capacity(rig.len());
        for index in 0..rig.len() {
            let name = rig.name(index);
            let entity = old
                .remove(name)
                .unwrap_or_else(|| commands.spawn(Name::new(name.to_owned())).id());
            let parent = rig.parent(index).map_or(root, |parent| entities[parent]);
            commands
                .entity(entity)
                .insert((Bone { puppet, index }, ChildOf(parent)));
            entities.push(entity);
        }
        for (_, gone) in old {
            commands.entity(gone).despawn();
        }
        commands.entity(puppet).insert(Bones {
            root,
            entities,
            rig: id,
        });
    }
}

/// Forgets how clips are bound, wherever a clip or a rig changed.
pub(crate) fn forget(
    mut clips: MessageReader<AssetEvent<Clip>>,
    mut rigs: MessageReader<AssetEvent<Rig>>,
    mut animators: Query<&mut Animator>,
) {
    let changed = clips.read().any(|event| {
        matches!(
            event,
            AssetEvent::Modified { .. } | AssetEvent::Removed { .. }
        )
    }) | rigs
        .read()
        .any(|event| matches!(event, AssetEvent::Modified { .. }));
    if changed {
        for mut animator in &mut animators {
            animator.forget();
        }
    }
}

/// What posing a puppet reads of it.
type Posed = (
    &'static Puppet,
    &'static mut Animator,
    &'static mut Pose,
    Option<&'static Params>,
    Option<&'static Gait>,
    Option<&'static Goals>,
);

/// Poses every puppet whose rig has loaded.
pub(crate) fn animate(
    time: Res<Time>,
    layers: Res<Layers>,
    rigs: Res<Assets<Rig>>,
    clips: Res<Assets<Clip>>,
    mut puppets: Query<Posed>,
) {
    let delta = time.delta_secs();
    for (Puppet(handle), mut animator, mut pose, params, gait, goals) in &mut puppets {
        let Some(rig) = rigs.get(handle) else {
            continue;
        };
        let inputs = crate::pose::animator_inputs(params, gait, goals);
        *pose = animator.pose(rig, handle.id(), &clips, &layers, inputs, delta);
    }
}

/// Places every puppet's bones from its pose: each bone's transform its joint
/// under its parent's, from where the pose lays them out on whole pixels; the
/// skeleton mirrored as it faces.
#[expect(
    clippy::cast_precision_loss,
    reason = "a difference of draw depths, far below where f32 loses integers"
)]
pub(crate) fn place(
    rigs: Res<Assets<Rig>>,
    puppets: Query<(&Puppet, &Pose, &Side, &Bones)>,
    mut bones: Query<(&mut Transform, &mut Placement), With<Bone>>,
    mut roots: Query<&mut Transform, (With<Skeleton>, Without<Bone>)>,
) {
    for (Puppet(handle), pose, side, entities) in &puppets {
        let Some(rig) = rigs.get(handle) else {
            continue;
        };
        if let Ok(mut root) = roots.get_mut(entities.root) {
            root.scale.x = side.sign();
        }
        let placed = rig.place(pose);
        for (index, (placement, entity)) in placed.iter().zip(entities.iter()).enumerate() {
            let Ok((mut transform, mut placed_on)) = bones.get_mut(entity) else {
                continue;
            };
            let parent = rig
                .parent(index)
                .map_or_else(Placement::default, |parent| placed[parent]);
            let local = Vec2::from_angle(-parent.angle).rotate(placement.at - parent.at);
            transform.translation = local.extend((placement.depth - parent.depth) as f32 * DEPTH);
            transform.rotation = turned(placement.angle - parent.angle);
            *placed_on = *placement;
        }
    }
}

/// `angle`, in radians, as a rotation: a whole number of quarter turns exactly,
/// so a sprite turned a quarter lands on whole pixels.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a whole number of quarter turns, rounded"
)]
fn turned(angle: f32) -> Quat {
    let quarters = (angle / FRAC_PI_2).round();
    if (angle - quarters * FRAC_PI_2).abs() < QUARTER {
        let half = std::f32::consts::FRAC_1_SQRT_2;
        match (quarters as i32).rem_euclid(4) {
            0 => Quat::IDENTITY,
            1 => Quat::from_xyzw(0.0, 0.0, half, half),
            2 => Quat::from_xyzw(0.0, 0.0, 1.0, 0.0),
            _ => Quat::from_xyzw(0.0, 0.0, -half, half),
        }
    } else {
        Quat::from_rotation_z(angle)
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;
    use bevy::transform::TransformPlugin;

    use super::*;
    use crate::PuppetPlugin;
    use crate::testing::{clip, rig};

    const ARM: &str = r#"(bones: {
        "torso": (at: (0, 10)),
        "arm": (parent: "torso", at: (4, 0)),
        "hand": (parent: "arm", at: (0, -6)),
    })"#;

    /// An app keeping puppets, with a puppet on `rig` playing `clips`.
    fn puppet(rig_source: &str, clips: &[&str]) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin));
        // What drawing reads: images and atlases.
        #[cfg(feature = "render")]
        app.add_plugins((
            bevy::image::ImagePlugin::default(),
            bevy::image::TextureAtlasPlugin,
        ));
        app.add_plugins(PuppetPlugin);
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Rig>>()
            .add(rig(rig_source).unwrap());
        let mut animator = Animator::default();
        for source in clips {
            let clip = app
                .world_mut()
                .resource_mut::<Assets<Clip>>()
                .add(clip(source));
            animator.play(&clip);
        }
        let puppet = app.world_mut().spawn((Puppet(handle), animator)).id();
        app.update();
        (app, puppet)
    }

    fn bone(app: &App, puppet: Entity, name: &str) -> Entity {
        let world = app.world();
        let rig = world
            .resource::<Assets<Rig>>()
            .get(&world.get::<Puppet>(puppet).unwrap().0)
            .unwrap();
        world
            .get::<Bones>(puppet)
            .unwrap()
            .named(rig, name)
            .unwrap()
    }

    fn at(app: &App, entity: Entity) -> Vec2 {
        app.world()
            .get::<GlobalTransform>(entity)
            .unwrap()
            .translation()
            .truncate()
    }

    #[test]
    fn builds_a_skeleton_of_bone_entities_each_under_its_parent() {
        let (app, puppet) = puppet(ARM, &[]);
        let world = app.world();
        let bones = world.get::<Bones>(puppet).unwrap();
        assert_eq!(world.get::<ChildOf>(bones.root()).unwrap().parent(), puppet);
        let [torso, arm, hand] = ["torso", "arm", "hand"].map(|name| bone(&app, puppet, name));
        assert_eq!(world.get::<ChildOf>(torso).unwrap().parent(), bones.root());
        assert_eq!(world.get::<ChildOf>(arm).unwrap().parent(), torso);
        assert_eq!(world.get::<ChildOf>(hand).unwrap().parent(), arm);
        assert_eq!(world.get::<Bone>(hand).unwrap().puppet(), puppet);
    }

    #[test]
    fn places_bones_where_the_pose_lays_them_out_and_mirrors_them() {
        // The arm turned a quarter: the hand, below it, out to its right.
        let turn = r#"(layer: Move, channels: [(bone: "arm", angle: Keys([(0, 90)]))])"#;
        let (mut app, puppet) = puppet(ARM, &[turn]);
        let hand = bone(&app, puppet, "hand");
        assert!(at(&app, hand).distance(Vec2::new(10.0, 10.0)) < 1e-4);
        app.world_mut().entity_mut(puppet).insert(Side::Left);
        app.update();
        assert!(at(&app, hand).distance(Vec2::new(-10.0, 10.0)) < 1e-4);
    }

    #[test]
    fn a_child_of_a_bone_moves_with_it() {
        let turn = r#"(layer: Move, channels: [(bone: "arm", angle: Keys([(0, 90)]))])"#;
        let (mut app, puppet) = puppet(ARM, &[turn]);
        let hand = bone(&app, puppet, "hand");
        // A blade a pixel up the hand, as the hand's art is drawn.
        let blade = app
            .world_mut()
            .spawn((Transform::from_xyz(0.0, 1.0, 0.0), ChildOf(hand)))
            .id();
        app.update();
        // The hand turned a quarter with the arm: up its art is to the left.
        assert!(at(&app, blade).distance(Vec2::new(9.0, 10.0)) < 1e-4);
    }

    #[test]
    fn a_changed_rig_keeps_its_bones_by_name_and_what_hangs_from_them() {
        let (mut app, puppet) = puppet(ARM, &[]);
        let hand = bone(&app, puppet, "hand");
        let held = app.world_mut().spawn(ChildOf(hand)).id();
        let id = app.world().get::<Puppet>(puppet).unwrap().0.id();
        let longer = r#"(bones: {
            "torso": (at: (0, 10)),
            "arm": (parent: "torso", at: (4, 0)),
            "hand": (parent: "arm", at: (0, -9)),
            "head": (parent: "torso", at: (0, 6)),
        })"#;
        app.world_mut()
            .resource_mut::<Assets<Rig>>()
            .insert(id, rig(longer).unwrap())
            .unwrap();
        app.update();
        app.update();
        assert_eq!(bone(&app, puppet, "hand"), hand);
        assert!(app.world().get_entity(held).is_ok());
        assert!(at(&app, hand).distance(Vec2::new(4.0, 1.0)) < 1e-4);
        assert_eq!(app.world().get::<Bones>(puppet).unwrap().iter().count(), 4);
    }
}
