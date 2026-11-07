# bevy_puppet

Cut-out puppets for Bevy: a skeleton of bone entities, posed every frame by
layered clips and constraints, and drawn by whatever the game parents under
its bones. One system for every body plan (a person, a spider, a bat), told
apart only by their rigs; made for pixel art: joints on whole pixels,
mirrored exactly, sprites pre-rotated with RotSprite where they turn. It
names nothing of any game: the game's own labels say what its roles,
layers, params and goals are.

```rust,ignore
use bevy::prelude::*;
use bevy_puppet::prelude::*;

#[derive(LayerLabel, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Layer { Move, Act }

#[derive(GoalLabel, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Want { Aim }

app.add_plugins((PuppetPlugin, PuppetRonPlugin::<Labels>::default()))
    .configure_puppet_layers([Layer::Move, Layer::Act]);
commands.spawn(Puppet(assets.load("hero.rig.ron")));

// In the game's systems, before `PuppetSystems`:
animator.play(&walk);
gait.advance(&walk_clip, walking, speed, delta);
goals.set(Want::Aim, Some(Goal::Angle(side.aim(direction))));
```

At the root: [`PuppetPlugin`] and [`PuppetSystems`], [`PuppetApp`], and the
puppet's entities: [`Puppet`], [`Skeleton`], [`Bone`] and [`Bones`]. The rest
is in modules, and [`prelude`] brings in what using puppets takes:

| Module | What's in it |
|---|---|
| [`rig`] | Body plans as assets: `Rig`, written as a `RigSource` (bones, roles, constraints) |
| [`clip`] | Ways a puppet moves, as assets: `Clip`, written as a `ClipSource`; channels on Bevy's curves |
| [`pose`] | What plays and what drives it: `Animator`, `Params`, `Goals`, `Gait`, `Side`; the `Pose` that comes of it |
| [`label`] | The game's labels: `RoleLabel`, `LayerLabel`, `ParamLabel`, `GoalLabel`, derived |
| `loader` | With `ron` (default): `.rig.ron` and `.clip.ron` loaders, in the game's labels (`PuppetLabels`) |

## Rigs and clips

A [`Rig`](rig::Rig) is a body plan: bones, each a part turning about a joint
(its parent, where its joint is, its depth, rest angle, limit, and whether it
turns in steps), the bone doing each role, and its constraints. A
[`Clip`](clip::Clip) is a way to move: channels turning or shifting bones,
by name or by role, so a clip made for any body (an ability's) finds its
bone on each. Both are assets, checked as they're made (`Rig::new`,
`Clip::new`), written in the game's own labels, and loaded from RON with
the `ron` feature; or built in code.

What advances a clip: time (looping, or once), ground covered (a
[`Gait`](pose::Gait) that finishes its step before it stands), or a param the
game sets (an action's progress). Curves are Bevy's: waves, keyframes, any
held in steps as drawn frames are.

## Posing

An [`Animator`](pose::Animator) holds what plays; the game plays and stops
clips on it. Each frame it plays them layer by layer, in the order the app
configures, each overriding or adding to those under it; after each layer,
the constraints that run after it, each toward its own goal
([`Goals`](pose::Goals)): `Aim` points a chain along an angle, each bone as
far as its limit lets and the next the rest; `Look` turns a bone a share of
one; `Reach` bends an arm so its hand reaches a point on another bone
(two-bone IK). Every bone kept to its limit, last. A clip that holds (a
death) plays over the pose it started from; `settle` holds halfway from the
last pose a moment, at a switch.

## Bones in the world

A [`Puppet`] gets a [`Skeleton`] child once its rig loads, and under it an
entity for each [`Bone`], each `ChildOf` its parent bone: a real hierarchy,
reflected, so inspectors show it and where each bone is, and anything the
game parents under a bone (a sprite, a blade, a particle emitter) moves,
turns and mirrors with it. Each frame
the pose is laid out with every joint on a whole pixel, rounded off its
parent's rounded joint, and written as the bones' transforms; facing left
([`Side`](pose::Side)) mirrors the skeleton exactly (its `scale.x`), so the
pixels stay put. A rig that changes (a hot reload) keeps its bones by name,
and what hangs from them; an entity no longer a puppet loses its skeleton.

## Drawing

What a puppet shows is the game's: sprites under its bones. With `render`
(default), a sprite under a bone that turns in steps is pre-rotated with
RotSprite, once, into even steps about its joint (its offset from the bone,
where its middle is), and shows the step nearest the bone's angle, upright,
so pixel art stays crisp and what hangs from it stays where its art is
drawn. Its anchor, flips and size are kept; a step an odd number of pixels
across sits half a pixel off the joint, its corner on a whole pixel. It's
stepped again as its bone's steps or its image change, and goes back as it
was once its bone no longer turns in steps. Without `render`, it's rigs, clips, posing and bone entities alone:
for a server.
