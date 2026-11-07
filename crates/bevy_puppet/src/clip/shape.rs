//! A channel's curve, ready to sample: one of Bevy's.

use std::f32::consts::TAU;
use std::fmt;
use std::sync::Arc;

use bevy_curve::{
    ConstantCurve, Curve as BevyCurve, FunctionCurve, Interval, UnevenSampleAutoCurve,
};

use super::source::{Clamp, Curve};

/// A curve a channel samples by its clip's parameter: any of Bevy's curves.
///
/// From data, or from code ([`Shape::new`]).
#[derive(Clone)]
pub struct Shape {
    curve: Arc<dyn BevyCurve<f32> + Send + Sync>,
    /// Whether it runs on by its own period, as a wave does, rather than
    /// playing over each cycle of a looping clip again.
    periodic: bool,
}

impl fmt::Debug for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shape")
            .field("domain", &self.curve.domain())
            .field("periodic", &self.periodic)
            .finish_non_exhaustive()
    }
}

impl Shape {
    /// `curve`, sampled over each cycle of its clip, from 0 to 1, held past its
    /// ends.
    pub fn new(curve: impl BevyCurve<f32> + Send + Sync + 'static) -> Self {
        Self {
            curve: Arc::new(curve),
            periodic: false,
        }
    }

    /// `curve`, as data writes it, if its keyframes are in order.
    pub(crate) fn from_curve(curve: &Curve) -> Option<Self> {
        if !curve.is_ordered() {
            return None;
        }
        Some(match curve {
            &Curve::Wave {
                amplitude,
                phase,
                frequency,
                clamp,
            } => Self {
                curve: Arc::new(FunctionCurve::new(Interval::EVERYWHERE, move |t| {
                    let value = amplitude * (TAU * (frequency * t + phase)).sin();
                    match clamp {
                        Some(Clamp::Above(bound)) => value.max(bound),
                        Some(Clamp::Below(bound)) => value.min(bound),
                        None => value,
                    }
                })),
                periodic: true,
            },
            Curve::Keys(keys) => match keys.as_slice() {
                [] => Self::new(ConstantCurve::new(Interval::EVERYWHERE, 0.0)),
                [(_, value)] => Self::new(ConstantCurve::new(Interval::EVERYWHERE, *value)),
                keys => Self::new(UnevenSampleAutoCurve::new(keys.iter().copied()).ok()?),
            },
        })
    }

    /// Its value at `t` on a clip that `loops` or doesn't: a wave runs on at
    /// its own frequency, round and round, whatever the clip's cycle, so any
    /// frequency loops without a seam; anything else plays over each cycle
    /// again.
    #[must_use]
    pub fn sample(&self, t: f32, loops: bool) -> f32 {
        let t = if loops && !self.periodic {
            t.rem_euclid(1.0)
        } else {
            t
        };
        self.curve.sample_clamped(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_clamp_to_one_side_and_keys_hold_past_their_ends() {
        let wave = Shape::from_curve(&Curve::Wave {
            amplitude: 10.0,
            phase: 0.0,
            frequency: 1.0,
            clamp: Some(Clamp::Above(0.0)),
        })
        .unwrap();
        assert!((wave.sample(0.25, true) - 10.0).abs() < 1e-4);
        assert_eq!(wave.sample(0.75, true), 0.0);
        let keys = Shape::from_curve(&Curve::Keys(vec![(0.0, 0.0), (1.0, 10.0)])).unwrap();
        assert!((keys.sample(0.5, false) - 5.0).abs() < 1e-4);
        assert_eq!(keys.sample(2.0, false), 10.0);
        // Looping, it plays over again.
        assert!((keys.sample(1.5, true) - 5.0).abs() < 1e-4);
        assert!(Shape::from_curve(&Curve::Keys(vec![(1.0, 0.0), (0.0, 1.0)])).is_none());
    }
}
