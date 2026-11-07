//! What plays on a puppet, and the pose it makes each frame.

use std::collections::HashMap;

use bevy::asset::{AssetId, Assets, Handle};
use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::log::warn;
use bevy::math::Vec2;

use super::{Gait, Goals, Params, Pose};
use crate::clip::{Binding, Clip, Drive};
use crate::label::{InternedLayer, LayerLabel};
use crate::rig::Rig;

/// The order clips' layers play in, bottom first, each over those under it.
///
/// Each overrides what those under it set, or adds to it. Configured with
/// [`configure_puppet_layers`](crate::PuppetApp::configure_puppet_layers); a
/// layer it doesn't name plays last.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct Layers(Vec<InternedLayer>);

/// What plays on a puppet: clips, by handle, posed every frame.
///
/// Each since it started. The game plays and stops them; each frame it poses
/// the puppet from them. A clip played before it has loaded plays from when it
/// was played, once it has.
#[derive(Component, Debug, Default, Clone)]
pub struct Animator {
    clock: f32,
    /// How many clips it's been told to play, all told: each playing's turn.
    plays: u64,
    playing: Vec<Playing>,
    /// Layers stopped while some clip played before was still loading, each
    /// with the plays before it was: kept until those clips' layers are known,
    /// and those on it taken off.
    stopped: Vec<(InternedLayer, u64)>,
    settle: Option<Settle>,
    last: Option<Pose>,
    /// The pose a held clip plays over: the layers under it as they were when
    /// it started.
    frozen: Option<Frozen>,
    /// Clips bound to the rig, by clip, with the revisions they were bound at.
    bound: HashMap<AssetId<Clip>, Bound>,
    /// The rig they were bound to, and its revision.
    rig: Option<(AssetId<Rig>, u64)>,
}

#[derive(Debug, Clone)]
struct Playing {
    clip: Handle<Clip>,
    since: f32,
    turn: u64,
    /// Its layer, once it's loaded.
    layer: Option<InternedLayer>,
    /// Whether its hold, if it holds, has frozen the pose.
    froze: bool,
}

#[derive(Debug, Clone)]
struct Frozen {
    pose: Pose,
    layer: InternedLayer,
    /// The turn of the clip holding it.
    owner: u64,
}

#[derive(Debug, Clone)]
struct Bound {
    revision: u64,
    binding: Option<Binding>,
}

#[derive(Debug, Clone)]
struct Settle {
    from: Pose,
    left: f32,
    fresh: bool,
}

/// What a frame's pose reads besides the clips playing.
#[derive(Clone, Copy, Default)]
pub(crate) struct Inputs<'a> {
    pub(crate) params: Option<&'a Params>,
    pub(crate) gait: Option<&'a Gait>,
    pub(crate) goals: Option<&'a Goals>,
}

impl Layers {
    /// `layers`, bottom first.
    pub fn new<L: LayerLabel>(layers: impl IntoIterator<Item = L>) -> Self {
        Self(layers.into_iter().map(|layer| layer.intern()).collect())
    }

    /// Where `layer` plays: its place, or after them all.
    fn rank(&self, layer: InternedLayer) -> usize {
        self.0
            .iter()
            .position(|named| *named == layer)
            .unwrap_or(self.0.len())
    }
}

impl Animator {
    /// Plays `clip` from its start, unless it's playing already.
    pub fn play(&mut self, clip: &Handle<Clip>) {
        if !self.playing(clip) {
            self.played(clip, 0.0);
        }
    }

    /// Plays `clip` from its start, again if it's playing.
    pub fn replay(&mut self, clip: &Handle<Clip>) {
        self.played(clip, 0.0);
    }

    /// Plays `clip` as if it started `ago` seconds ago: one seen only once it's
    /// under way, or over (a death long since).
    pub fn played(&mut self, clip: &Handle<Clip>, ago: f32) {
        self.stop(clip);
        self.plays += 1;
        self.playing.push(Playing {
            clip: clip.clone(),
            since: self.clock - ago,
            turn: self.plays,
            layer: None,
            froze: false,
        });
    }

    /// Stops `clip`.
    pub fn stop(&mut self, clip: &Handle<Clip>) {
        self.playing.retain(|playing| playing.clip != *clip);
    }

    /// Stops every clip playing on `layer`, at once; a clip still loading, as
    /// soon as its layer is known to be `layer`.
    pub fn stop_layer(&mut self, layer: impl LayerLabel) {
        let layer = layer.intern();
        self.playing.retain(|playing| playing.layer != Some(layer));
        if self.playing.iter().any(|playing| playing.layer.is_none()) {
            self.stopped.push((layer, self.plays));
        }
    }

    /// Whether `clip` is playing: not if a layer stopped since may be its.
    #[must_use]
    pub fn playing(&self, clip: &Handle<Clip>) -> bool {
        self.playing
            .iter()
            .any(|playing| playing.clip == *clip && !self.maybe_stopped(playing))
    }

    /// Holds halfway from the last pose for `seconds`: a switch between clips
    /// (a walk to a stand), eased.
    pub fn settle(&mut self, seconds: f32) {
        if let Some(from) = self.last.clone() {
            self.settle = Some(Settle {
                from,
                left: seconds,
                fresh: true,
            });
        }
    }

    /// Whether `playing`, still loading, was played before a layer was stopped
    /// that may be its.
    fn maybe_stopped(&self, playing: &Playing) -> bool {
        playing.layer.is_none()
            && self
                .stopped
                .iter()
                .any(|&(_, before)| playing.turn <= before)
    }

    /// `delta` seconds on, the pose of `rig` from what plays and `inputs`:
    /// layer by layer, each one's clips and then its constraints; every bone
    /// kept to its limit; a held clip over the pose it started from; eased
    /// halfway from the last pose while it settles.
    pub(crate) fn pose(
        &mut self,
        rig: &Rig,
        rig_id: AssetId<Rig>,
        clips: &Assets<Clip>,
        layers: &Layers,
        inputs: Inputs,
        delta: f32,
    ) -> Pose {
        if self.rig != Some((rig_id, rig.revision)) {
            self.bound.clear();
            self.frozen = None;
            self.rig = Some((rig_id, rig.revision));
        }
        self.learn(clips);
        // A clip that holds freezes the layers under it as they were when
        // it was played: as of the last frame.
        self.hold(rig, clips, layers, inputs);
        self.clock += delta;
        let clock = self.clock;
        self.playing.retain(|playing| {
            clips
                .get(&playing.clip)
                .is_none_or(|clip| clip.hold || !clip.finished(clock - playing.since))
        });
        let (mut pose, floor) = match &self.frozen {
            Some(frozen) if frozen.pose.joints.len() == rig.len() => {
                (frozen.pose.clone(), layers.rank(frozen.layer))
            }
            _ => (Pose::rest(rig), 0),
        };
        self.build(rig, clips, layers, inputs, &mut pose, floor..usize::MAX);
        for (bone, joint) in rig.bones.iter().zip(&mut pose.joints) {
            joint.angle = bone.limited(joint.angle);
        }
        let playing: Vec<AssetId<Clip>> = self
            .playing
            .iter()
            .map(|playing| playing.clip.id())
            .collect();
        self.bound.retain(|id, _| playing.contains(id));
        let pose = match &mut self.settle {
            Some(settle) => {
                if settle.fresh {
                    settle.fresh = false;
                } else {
                    settle.left -= delta;
                }
                if settle.left > 0.0 && settle.from.joints.len() == pose.joints.len() {
                    settle.from.between(&pose, 0.5)
                } else {
                    self.settle = None;
                    pose
                }
            }
            None => pose,
        };
        self.last = Some(pose.clone());
        pose
    }

    /// Learns the layers of clips that have loaded, taking off those a layer
    /// stopped since they were played; and forgets stops that no clip still
    /// loading can be under.
    fn learn(&mut self, clips: &Assets<Clip>) {
        let stopped = self.stopped.clone();
        self.playing.retain_mut(|playing| {
            if playing.layer.is_none()
                && let Some(clip) = clips.get(&playing.clip)
            {
                playing.layer = Some(clip.layer);
                if stopped
                    .iter()
                    .any(|&(layer, before)| layer == clip.layer && playing.turn <= before)
                {
                    return false;
                }
            }
            true
        });
        let loading: Vec<u64> = self
            .playing
            .iter()
            .filter(|playing| playing.layer.is_none())
            .map(|playing| playing.turn)
            .collect();
        self.stopped
            .retain(|&(_, before)| loading.iter().any(|&turn| turn <= before));
    }

    /// Freezes the pose for a clip that holds as it starts: the layers under it
    /// as they are now. Lets it go once its clip stops; another still holding
    /// freezes it again.
    fn hold(&mut self, rig: &Rig, clips: &Assets<Clip>, layers: &Layers, inputs: Inputs) {
        if let Some(frozen) = &self.frozen
            && !self
                .playing
                .iter()
                .any(|playing| playing.turn == frozen.owner)
        {
            self.frozen = None;
            for playing in &mut self.playing {
                playing.froze = false;
            }
        }
        let fresh = self.playing.iter().position(|playing| {
            !playing.froze && clips.get(&playing.clip).is_some_and(|clip| clip.hold)
        });
        let Some(index) = fresh else {
            return;
        };
        let playing = &mut self.playing[index];
        playing.froze = true;
        let turn = playing.turn;
        let Some(layer) = clips.get(&playing.clip).map(|clip| clip.layer) else {
            return;
        };
        let mut below = Pose::rest(rig);
        self.build(
            rig,
            clips,
            layers,
            inputs,
            &mut below,
            0..layers.rank(layer),
        );
        self.frozen = Some(Frozen {
            pose: below,
            layer,
            owner: turn,
        });
    }

    /// Plays the layers ranked within `ranks` onto `pose`, each followed by its
    /// constraints; and, if `ranks` runs past them all, the clips and
    /// constraints of layers the app doesn't name.
    fn build(
        &mut self,
        rig: &Rig,
        clips: &Assets<Clip>,
        layers: &Layers,
        inputs: Inputs,
        pose: &mut Pose,
        ranks: std::ops::Range<usize>,
    ) {
        for (rank, &layer) in layers.0.iter().enumerate() {
            if !ranks.contains(&rank) {
                continue;
            }
            self.layer(rig, clips, pose, |clip| clip.layer == layer, inputs);
            Self::constrain(rig, pose, inputs, |after| after == layer);
        }
        if ranks.contains(&layers.0.len()) {
            self.layer(
                rig,
                clips,
                pose,
                |clip| !layers.0.contains(&clip.layer),
                inputs,
            );
            Self::constrain(rig, pose, inputs, |after| !layers.0.contains(&after));
        }
    }

    /// Plays the clips `on` picks onto `pose`, in the order they started.
    fn layer(
        &mut self,
        rig: &Rig,
        clips: &Assets<Clip>,
        pose: &mut Pose,
        on: impl Fn(&Clip) -> bool,
        inputs: Inputs,
    ) {
        for index in 0..self.playing.len() {
            let playing = &self.playing[index];
            let Some(clip) = clips.get(&playing.clip) else {
                continue;
            };
            if !on(clip) {
                continue;
            }
            let id = playing.clip.id();
            let t = self.parameter(clip, playing.since, inputs);
            let bind = || Bound {
                revision: clip.revision,
                binding: rig
                    .bind(clip)
                    .inspect_err(|error| warn!("a clip doesn't fit its puppet's rig: {error}"))
                    .ok(),
            };
            let bound = self.bound.entry(id).or_insert_with(bind);
            if bound.revision != clip.revision {
                *bound = bind();
            }
            if let Some(binding) = &bound.binding {
                Self::apply(rig, pose, clip, binding, t);
            }
        }
    }

    /// Runs the constraints that run after a layer `after` picks.
    fn constrain(
        rig: &Rig,
        pose: &mut Pose,
        inputs: Inputs,
        after: impl Fn(InternedLayer) -> bool,
    ) {
        let Some(goals) = inputs.goals else {
            return;
        };
        for constraint in rig
            .constraints
            .iter()
            .filter(|constraint| after(constraint.after))
        {
            if let Some(goal) = goals.value(constraint.goal) {
                constraint.solve(rig, pose, goal);
            }
        }
    }

    /// Where `clip` is: its parameter, held to its steps.
    fn parameter(&self, clip: &Clip, since: f32, inputs: Inputs) -> f32 {
        let t = match clip.driver {
            Drive::Time { length, loops } => {
                let t = (self.clock - since) / length.max(f32::EPSILON);
                if loops { t } else { t.min(1.0) }
            }
            Drive::Gait { .. } => inputs.gait.map_or(0.0, |gait| gait.cycles),
            Drive::Param { param, from, to } => {
                let value = inputs.params.map_or(0.0, |params| params.value(param));
                if (to - from).abs() > f32::EPSILON {
                    (value - from) / (to - from)
                } else {
                    value
                }
            }
        };
        clip.step(t)
    }

    /// Sets (or adds) each of `clip`'s channels at `t` on its bone.
    fn apply(rig: &Rig, pose: &mut Pose, clip: &Clip, binding: &Binding, t: f32) {
        let loops = clip.loops();
        for (channel, &index) in clip.channels.iter().zip(&binding.0) {
            let (Some(bone), true) = (rig.bones.get(index), index < pose.joints.len()) else {
                continue;
            };
            let joint = &mut pose.joints[index];
            let set = |slot: &mut f32, value: f32| {
                if channel.add {
                    *slot += value;
                } else {
                    *slot = value;
                }
            };
            if let Some(degrees) = channel.angle.as_ref().map(|shape| shape.sample(t, loops)) {
                set(&mut joint.angle, degrees.to_radians());
            }
            if let Some(x) = channel.x.as_ref().map(|shape| shape.sample(t, loops)) {
                set(&mut joint.shift.x, x);
            }
            if let Some(y) = channel.y.as_ref().map(|shape| shape.sample(t, loops)) {
                set(&mut joint.shift.y, y);
            }
            let direction = Vec2::from_angle(joint.angle + bone.rest + bone.hang);
            joint.shift += channel.extended(t, loops, direction);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use bevy::math::Vec2;

    use super::*;
    use crate::pose::{Goal, Goals, Params};
    use crate::testing::{Job, Knob, Mark, Stage, Tier, near, rig};

    /// A body with an arm, an off arm and a front leg.
    fn body(constraints: &str) -> Stage {
        Stage::new(
            rig(&format!(
                r#"(
                    bones: {{
                        "torso": (at: (0, 20)),
                        "arm": (parent: "torso", at: (2, 6)),
                        "forearm": (parent: "arm", at: (0, -6)),
                        "hand": (parent: "forearm", at: (0, -6)),
                        "off_arm": (parent: "torso", at: (-2, 6)),
                        "off_forearm": (parent: "off_arm", at: (0, -6)),
                        "off_hand": (parent: "off_forearm", at: (0, -6)),
                        "front": (parent: "torso", at: (0, -8)),
                    }},
                    roles: {{Reach: "arm", Grip: "hand", Off: "off_hand", Body: "torso"}},
                    constraints: [{constraints}],
                )"#
            ))
            .unwrap(),
        )
    }

    fn aim(angle: f32) -> Goals {
        let mut goals = Goals::default();
        goals.set(Mark::Aim, Some(Goal::Angle(angle)));
        goals
    }

    #[test]
    fn higher_layers_override_or_add_to_lower_ones() {
        let mut stage = body("");
        let torso = stage.index("torso");
        let idle =
            stage.clip(r#"(layer: Move, channels: [(bone: "torso", angle: Keys([(0, 5)]))])"#);
        let lean = stage.clip(
            r#"(layer: Act, channels: [(bone: "torso", add: true, angle: Keys([(0, 10)]))])"#,
        );
        let flinch = stage.clip(
            r#"(layer: React, driver: Time(length: 1), channels: [(bone: "torso", angle: Keys([(0, -8)]))])"#,
        );
        let mut animator = Animator::default();
        animator.play(&lean);
        animator.play(&idle);
        let pose = stage.pose(&mut animator, None, None, None, 0.01);
        assert!(near(pose.joints[torso].angle, 15.0));
        animator.play(&flinch);
        let pose = stage.pose(&mut animator, None, None, None, 0.01);
        assert!(near(pose.joints[torso].angle, -8.0));
        let pose = stage.pose(&mut animator, None, None, None, 2.0);
        assert!(near(pose.joints[torso].angle, 15.0));
        assert!(!animator.playing(&flinch));
    }

    #[test]
    fn stopping_a_layer_leaves_what_plays_after_it() {
        let mut stage = body("");
        let front = stage.index("front");
        let step =
            stage.clip(r#"(layer: Move, channels: [(bone: "front", angle: Keys([(0, 30)]))])"#);
        let stand =
            stage.clip(r#"(layer: Move, channels: [(bone: "front", angle: Keys([(0, 0)]))])"#);
        let mut animator = Animator::default();
        animator.play(&step);
        stage.pose(&mut animator, None, None, None, 0.1);
        // Stopped, then played again on the same layer before the next pose.
        animator.stop_layer(Tier::Move);
        animator.play(&stand);
        stage.pose(&mut animator, None, None, None, 0.1);
        assert!(!animator.playing(&step));
        assert!(animator.playing(&stand));
        let pose = stage.pose(&mut animator, None, None, None, 0.1);
        assert!(near(pose.joints[front].angle, 0.0));
    }

    #[test]
    fn stopping_a_layer_then_playing_one_of_its_clips_plays_it() {
        let mut stage = body("");
        let front = stage.index("front");
        let walk =
            stage.clip(r#"(layer: Move, channels: [(bone: "front", angle: Keys([(0, 30)]))])"#);
        let mut animator = Animator::default();
        animator.play(&walk);
        stage.pose(&mut animator, None, None, None, 0.1);
        animator.stop_layer(Tier::Move);
        assert!(!animator.playing(&walk), "stopped at once");
        animator.play(&walk);
        let pose = stage.pose(&mut animator, None, None, None, 0.1);
        assert!(animator.playing(&walk));
        assert!(near(pose.joints[front].angle, 30.0));
    }

    #[test]
    fn a_layer_stopped_while_a_clip_loads_stops_it_once_it_has() {
        let mut stage = body("");
        let torso = stage.index("torso");
        let flinch = stage.loading();
        let idle = stage.loading();
        let mut animator = Animator::default();
        animator.play(&flinch);
        animator.play(&idle);
        animator.stop_layer(Tier::React);
        stage.pose(&mut animator, None, None, None, 0.1);
        stage.load(
            &flinch,
            r#"(layer: React, channels: [(bone: "torso", angle: Keys([(0, -8)]))])"#,
        );
        stage.load(
            &idle,
            r#"(layer: Move, channels: [(bone: "torso", angle: Keys([(0, 5)]))])"#,
        );
        let pose = stage.pose(&mut animator, None, None, None, 0.1);
        assert!(!animator.playing(&flinch), "its layer was stopped");
        assert!(animator.playing(&idle), "another layer's plays on");
        assert!(near(pose.joints[torso].angle, 5.0));
    }

    #[test]
    fn a_clip_replaced_is_bound_again() {
        let mut stage = body("");
        let (torso, front) = (stage.index("torso"), stage.index("front"));
        let lean =
            stage.clip(r#"(layer: Move, channels: [(bone: "torso", angle: Keys([(0, 5)]))])"#);
        let mut animator = Animator::default();
        animator.play(&lean);
        stage.pose(&mut animator, None, None, None, 0.1);
        stage.load(
            &lean,
            r#"(layer: Move, channels: [(bone: "front", angle: Keys([(0, 7)]))])"#,
        );
        let pose = stage.pose(&mut animator, None, None, None, 0.1);
        assert!(near(pose.joints[front].angle, 7.0));
        assert!(near(pose.joints[torso].angle, 0.0));
    }

    #[test]
    fn a_held_clip_adds_what_plays_over_it_once() {
        let mut stage = body("");
        let torso = stage.index("torso");
        let breathe = stage.clip(
            r#"(layer: Hold, channels: [(bone: "torso", add: true, angle: Keys([(0, 4)]))])"#,
        );
        let death = stage.clip(
            r#"(layer: React, driver: Time(length: 0.5), hold: true, channels: [(bone: "front", angle: Keys([(0, 90)]))])"#,
        );
        let mut animator = Animator::default();
        animator.play(&breathe);
        stage.pose(&mut animator, None, None, None, 0.1);
        animator.replay(&death);
        for _ in 0..10 {
            stage.pose(&mut animator, None, None, None, 0.1);
        }
        let dead = stage.pose(&mut animator, None, None, None, 0.1);
        assert!(
            near(dead.joints[torso].angle, 4.0),
            "breathing counted once"
        );
    }

    #[test]
    fn settles_halfway_for_a_frame_at_a_switch() {
        let mut stage = body("");
        let front = stage.index("front");
        let step =
            stage.clip(r#"(layer: Move, channels: [(bone: "front", angle: Keys([(0, 30)]))])"#);
        let stand =
            stage.clip(r#"(layer: Move, channels: [(bone: "front", angle: Keys([(0, 0)]))])"#);
        let mut animator = Animator::default();
        animator.play(&step);
        stage.pose(&mut animator, None, None, None, 0.1);
        animator.stop_layer(Tier::Move);
        animator.play(&stand);
        animator.settle(0.08);
        let settling = stage.pose(&mut animator, None, None, None, 0.0);
        assert!(near(settling.joints[front].angle, 15.0));
        let settled = stage.pose(&mut animator, None, None, None, 0.1);
        assert!(near(settled.joints[front].angle, 0.0));
    }

    #[test]
    fn a_held_clip_owns_the_pose_it_started_from() {
        let mut stage = body("");
        let (torso, front) = (stage.index("torso"), stage.index("front"));
        let walk = stage.clip(
            r#"(layer: Move, driver: Time(length: 1, loops: true), channels: [(bone: "front", angle: Wave(amplitude: 30))])"#,
        );
        let death = stage.clip(
            r#"(layer: React, driver: Time(length: 0.5), hold: true, channels: [(bone: "torso", angle: Keys([(0, 0), (1, 90)]))])"#,
        );
        let mut animator = Animator::default();
        animator.play(&walk);
        let walking = stage.pose(&mut animator, None, None, None, 0.1);
        animator.replay(&death);
        // Long after, still walking: the torso fallen and held, the legs
        // where they were when it died, nothing stepping.
        for _ in 0..30 {
            stage.pose(&mut animator, None, None, None, 0.1);
        }
        let dead = stage.pose(&mut animator, None, None, None, 0.1);
        assert!(near(dead.joints[torso].angle, 90.0));
        assert_eq!(dead.joints[front].angle, walking.joints[front].angle);
        // Seen once it's over, it lies as it ended.
        let mut seen = Animator::default();
        seen.played(&death, 10.0);
        let lying = stage.pose(&mut seen, None, None, None, 0.01);
        assert!(near(lying.joints[torso].angle, 90.0));
    }

    #[test]
    fn params_drive_what_reads_them() {
        let mut stage = body("");
        let torso = stage.index("torso");
        let rise = stage.clip(
            r#"(layer: Move, driver: Param(param: Rise, from: 100, to: -100), channels: [(bone: "torso", angle: Keys([(0, 0), (1, 20)]))])"#,
        );
        let mut animator = Animator::default();
        animator.play(&rise);
        let mut params = Params::default();
        // Unset: halfway, neither rising nor falling.
        let pose = stage.pose(&mut animator, Some(&params), None, None, 0.01);
        assert!(near(pose.joints[torso].angle, 10.0));
        params.set(Knob::Rise, Some(-100.0));
        let pose = stage.pose(&mut animator, Some(&params), None, None, 0.01);
        assert!(near(pose.joints[torso].angle, 20.0));
    }

    #[test]
    fn aims_and_adds_its_action_to_the_aim() {
        let mut stage = body(r#"Aim(chain: ["arm"], goal: Aim, after: Carry)"#);
        let arm = stage.index("arm");
        let goals = aim(0.0);
        let mut animator = Animator::default();
        let pose = stage.pose(&mut animator, None, None, Some(&goals), 0.01);
        // Drawn hanging down, pointing ahead is a quarter turn up.
        assert!((pose.joints[arm].angle - FRAC_PI_2).abs() < 1e-5);
        let swing = stage.clip(
            "(layer: Act, driver: Param(param: Progress), channels: [(role: Reach, add: true, angle: Keys([(0, 0), (1, 60)]))])",
        );
        let mut params = Params::default();
        params.set(Knob::Progress, Some(1.0));
        animator.play(&swing);
        let pose = stage.pose(&mut animator, Some(&params), None, Some(&goals), 0.01);
        assert!(near(pose.joints[arm].angle, 150.0));
    }

    #[test]
    fn each_constraint_reaches_for_its_own_goal() {
        let stage = Stage::new(
            rig(r#"(
                bones: {
                    "torso": (at: (0, 20)),
                    "arm": (parent: "torso", at: (2, 6)),
                    "head": (parent: "torso", at: (0, 10)),
                },
                constraints: [
                    Aim(chain: ["arm"], goal: Aim, after: Carry),
                    Look(bone: "head", share: 1, goal: Gaze, after: Carry),
                ],
            )"#)
            .unwrap(),
        );
        let (arm, head) = (stage.index("arm"), stage.index("head"));
        // Aiming ahead, looking up: each its own way.
        let mut goals = aim(0.0);
        goals.set(Mark::Gaze, Some(Goal::Angle(FRAC_PI_2)));
        let pose = stage.aimed(&goals);
        assert!(near(pose.joints[arm].angle, 90.0));
        assert!(near(pose.joints[head].angle, 90.0));
        // A goal unset, or of the other kind: its constraint does nothing.
        goals.set(Mark::Gaze, None);
        goals.set(Mark::Aim, Some(Goal::Point(Vec2::ONE)));
        let pose = stage.aimed(&goals);
        assert!(near(pose.joints[arm].angle, 0.0));
        assert!(near(pose.joints[head].angle, 0.0));
    }

    #[test]
    fn aims_a_chain_within_its_limits_and_looks_its_share() {
        let mut stage = Stage::new(
            rig(r#"(
                bones: {
                    "torso": (at: (0, 20)),
                    "arm": (parent: "torso", at: (2, 6), limit: (min: -60, max: 120)),
                    "forearm": (parent: "arm", at: (0, -10), limit: (min: 0, max: 150)),
                    "head": (parent: "torso", at: (0, 10), limit: (min: -15, max: 15)),
                },
                constraints: [
                    Aim(chain: ["arm", "forearm"], goal: Aim, after: Carry),
                    Look(bone: "head", share: 0.25, goal: Aim, after: Carry),
                ],
            )"#)
            .unwrap(),
        );
        let [arm, forearm, head] = ["arm", "forearm", "head"].map(|name| stage.index(name));
        // A little up: the arm along it alone (drawn hanging, a quarter turn
        // more), the head a quarter of it.
        let pose = stage.aimed(&aim(20f32.to_radians()));
        assert!(near(pose.joints[arm].angle, 110.0));
        assert!(near(pose.joints[forearm].angle, 0.0));
        assert!(near(pose.joints[head].angle, 5.0));
        // Straight up: the arm stops 30 up, the forearm bends the rest; the
        // head stops at its limit.
        let pose = stage.aimed(&aim(FRAC_PI_2));
        assert!(near(pose.joints[arm].angle, 120.0));
        assert!(near(pose.joints[forearm].angle, 60.0));
        assert!(near(pose.joints[head].angle, 15.0));
        // Not aiming, a clip past the arm's limit: kept to it.
        let past =
            stage.clip(r#"(layer: Move, channels: [(bone: "arm", angle: Keys([(0, 200)]))])"#);
        let mut animator = Animator::default();
        animator.play(&past);
        let pose = stage.pose(&mut animator, None, None, None, 0.01);
        assert!(near(pose.joints[arm].angle, 120.0));
    }

    #[test]
    fn points_what_the_hand_grips_along_the_aim() {
        // A forearm drawn leaning 17 ahead of hanging, its hand gripping a
        // blade turned 45 up out of the fist.
        let stage = Stage::new(
            rig(r#"(
                bones: {
                    "torso": (at: (0, 20)),
                    "arm": (parent: "torso", at: (2, 6), limit: (min: -60, max: 120)),
                    "forearm": (parent: "arm", at: (0, -10), hang: -73),
                    "hand": (parent: "forearm", at: (3, -10), rest: 62),
                },
                constraints: [Aim(chain: ["arm", "forearm", "hand"], goal: Aim, after: Carry)],
            )"#)
            .unwrap(),
        );
        let [arm, forearm, hand] = ["arm", "forearm", "hand"].map(|name| stage.index(name));
        // Aimed level: the arm turns at the shoulder alone, 28 ahead of
        // hanging, the elbow and the grip as drawn.
        let pose = stage.aimed(&aim(0.0));
        assert!(near(pose.joints[arm].angle, 28.0));
        assert!(near(pose.joints[forearm].angle, 0.0));
        assert!(near(pose.joints[hand].angle, 0.0));
        let blade = pose.joints[arm].angle + pose.joints[forearm].angle + 62f32.to_radians();
        assert!(near(blade - FRAC_PI_2, 0.0));
    }

    #[test]
    fn aiming_low_past_its_limits_the_wrist_turns_the_rest() {
        let mut stage = Stage::new(
            rig(r#"(
                bones: {
                    "torso": (at: (0, 20)),
                    "arm": (parent: "torso", at: (2, 6), limit: (min: 0, max: 120)),
                    "forearm": (parent: "arm", at: (0, -10), hang: -73, limit: (min: -17, max: 150)),
                    "hand": (parent: "forearm", at: (3, -13), rest: 107, limit: (min: -90, max: 0)),
                },
                constraints: [Aim(chain: ["arm", "forearm", "hand"], goal: Aim, after: Carry)],
            )"#)
            .unwrap(),
        );
        let [arm, forearm, hand] = ["arm", "forearm", "hand"].map(|name| stage.index(name));
        let goals = aim(-FRAC_PI_2);
        let mut animator = Animator::default();
        let pose = stage.pose(&mut animator, None, None, Some(&goals), 0.01);
        assert!(near(pose.joints[arm].angle, 0.0));
        assert!(near(pose.joints[forearm].angle, -17.0));
        assert!(near(pose.joints[hand].angle, -90.0));
        // A follow-through turning the wrist on past straight: kept to it.
        let swing = stage.clip(
            r#"(layer: Act, driver: Param(param: Progress), channels: [(bone: "hand", add: true, angle: Keys([(0, -90)]))])"#,
        );
        animator.play(&swing);
        let pose = stage.pose(&mut animator, None, None, Some(&goals), 0.01);
        assert!(near(pose.joints[hand].angle, -90.0));
    }

    #[test]
    fn reaches_its_off_hand_to_the_grip() {
        let stage = body(
            r#"Aim(chain: ["arm"], goal: Aim, after: Carry), Reach(hand: "off_hand", on: "hand", goal: Hold, after: React)"#,
        );
        // Back up the held arm, within the off arm's reach.
        let at = Vec2::new(0.0, 10.0);
        let mut goals = aim(0.0);
        goals.set(Mark::Hold, Some(Goal::Point(at)));
        let pose = stage.aimed(&goals);
        let joints = stage.rig.joints(&pose);
        let (grip, angle) = joints[stage.rig.role(Job::Grip).unwrap()];
        let target = grip + Vec2::from_angle(angle).rotate(at);
        let (off, _) = joints[stage.rig.role(Job::Off).unwrap()];
        assert!(off.distance(target) < 0.01, "{off} vs {target}");
    }
}
