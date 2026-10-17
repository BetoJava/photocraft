//! Straight painting symmetry axes. Geometry is independent of pixels and the UI.

use crate::StrokePoint;
use serde::{Deserialize, Serialize};

pub const MAX_COORDINATE: f64 = 1_000_000.0;
/// Mandala mirror line count bounds: fewer than two lines is plain mirroring, and every line
/// costs two render passes per stroke.
pub const MIN_AXES: u8 = 2;
pub const MAX_AXES: u8 = 16;
pub const DEFAULT_AXES: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymmetryMode {
    Vertical,
    Horizontal,
    Dual,
    Diagonal,
    /// `axes` mirror lines through the centre, 180°/axes apart (the dihedral group D_axes).
    Mandala,
}

impl SymmetryMode {
    pub const ALL: [Self; 5] = [Self::Vertical, Self::Horizontal, Self::Dual, Self::Diagonal, Self::Mandala];
    pub fn id(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
            Self::Dual => "dual",
            Self::Diagonal => "diagonal",
            Self::Mandala => "mandala",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PresetSymmetry {
    pub mode: SymmetryMode,
    pub center: [f64; 2],
    /// Clockwise degrees relative to the mode's initial direction (document Y points down).
    pub rotation: f64,
    /// Mirror line count: fixed by the mode except for Mandala (MIN_AXES..=MAX_AXES).
    pub axes: u8,
}

impl PresetSymmetry {
    pub fn new(mode: SymmetryMode, center: [f64; 2], rotation: f64) -> Option<Self> {
        if !rotation.is_finite() || !center.iter().all(|x| x.is_finite() && x.abs() <= MAX_COORDINATE) {
            return None;
        }
        let axes = match mode {
            SymmetryMode::Dual => 2,
            SymmetryMode::Mandala => DEFAULT_AXES,
            _ => 1,
        };
        Some(Self { mode, center, rotation: rotation.rem_euclid(360.0), axes })
    }
    /// Set the Mandala line count; `None` when out of range. Other modes keep their fixed count.
    pub fn with_axes(self, axes: u8) -> Option<Self> {
        match self.mode {
            SymmetryMode::Mandala if (MIN_AXES..=MAX_AXES).contains(&axes) => Some(Self { axes, ..self }),
            SymmetryMode::Mandala => None,
            _ => Some(self),
        }
    }
    pub fn directions(self) -> Vec<[f64; 2]> {
        let initial: f64 = match self.mode {
            SymmetryMode::Horizontal => 0.0,
            SymmetryMode::Diagonal => 45.0,
            _ => 90.0,
        };
        let a = (initial + self.rotation).to_radians();
        let first = [a.cos(), a.sin()];
        match self.mode {
            SymmetryMode::Dual => vec![first, [-first[1], first[0]]],
            SymmetryMode::Mandala => {
                let step = 180.0 / f64::from(self.axes.max(1));
                (0..self.axes).map(|k| (initial + self.rotation + step * f64::from(k)).to_radians()).map(|a| [a.cos(), a.sin()]).collect()
            }
            _ => vec![first],
        }
    }
    /// Copies painted besides the original stroke: Dual and Mandala add the rotations that
    /// composing two reflections produces (N reflections + N-1 rotations for N lines).
    pub fn mirror_count(self) -> usize {
        match self.mode {
            SymmetryMode::Dual => 3,
            SymmetryMode::Mandala => 2 * usize::from(self.axes.max(1)) - 1,
            _ => 1,
        }
    }
    pub fn reflected_passes(self, points: &[StrokePoint]) -> Vec<Vec<StrokePoint>> {
        let reflect = |p: &StrokePoint, direction: [f64; 2]| {
            let [dx, dy] = [p.x - self.center[0], p.y - self.center[1]];
            let along = dx * direction[0] + dy * direction[1];
            StrokePoint { x: self.center[0] + 2.0 * along * direction[0] - dx, y: self.center[1] + 2.0 * along * direction[1] - dy, ..*p }
        };
        let mut copies: Vec<_> = self.directions().into_iter().map(|d| points.iter().map(|p| reflect(p, d)).collect()).collect();
        if self.mode == SymmetryMode::Dual {
            copies.push(points.iter().map(|p| StrokePoint { x: 2.0 * self.center[0] - p.x, y: 2.0 * self.center[1] - p.y, ..*p }).collect());
        }
        if self.mode == SymmetryMode::Mandala {
            let n = f64::from(self.axes.max(1));
            for k in 1..self.axes {
                let (sin, cos) = (std::f64::consts::TAU * f64::from(k) / n).sin_cos();
                copies.push(
                    points
                        .iter()
                        .map(|p| {
                            let [dx, dy] = [p.x - self.center[0], p.y - self.center[1]];
                            StrokePoint { x: self.center[0] + dx * cos - dy * sin, y: self.center[1] + dx * sin + dy * cos, ..*p }
                        })
                        .collect(),
                );
            }
        }
        copies
    }
    /// Clip infinite axes to document bounds without assuming that their centre is inside.
    pub fn segments(self, size: [f64; 2]) -> Vec<[[f64; 2]; 2]> {
        self.directions()
            .into_iter()
            .filter_map(|d| {
                let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
                for ((origin, direction), extent) in self.center.into_iter().zip(d).zip(size) {
                    if direction.abs() < 1e-12 {
                        if origin < 0.0 || origin > extent {
                            return None;
                        }
                    } else {
                        let (a, b) = (-origin / direction, (extent - origin) / direction);
                        lo = lo.max(a.min(b));
                        hi = hi.min(a.max(b));
                    }
                }
                if !lo.is_finite() || !hi.is_finite() || hi < lo {
                    return None;
                }
                Some([[self.center[0] + lo * d[0], self.center[1] + lo * d[1]], [self.center[0] + hi * d[0], self.center[1] + hi * d[1]]])
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modes_reflect_about_their_centre_and_keep_stylus_data() {
        let p = StrokePoint { x: 12.0, y: 25.0, pressure: 0.4, tilt_x: 18.0, ..Default::default() };
        for (mode, expected) in [(SymmetryMode::Vertical, [28.0, 25.0]), (SymmetryMode::Horizontal, [12.0, 35.0]), (SymmetryMode::Diagonal, [15.0, 22.0])] {
            let s = PresetSymmetry::new(mode, [20.0, 30.0], 0.0).unwrap();
            let r = s.reflected_passes(&[p])[0][0];
            assert!((r.x - expected[0]).abs() < 1e-9 && (r.y - expected[1]).abs() < 1e-9);
            assert_eq!((r.pressure, r.tilt_x), (p.pressure, p.tilt_x));
        }
        let s = PresetSymmetry::new(SymmetryMode::Dual, [20.0, 30.0], 0.0).unwrap();
        let copies = s.reflected_passes(&[p]);
        assert_eq!(copies.len(), 3);
        for (p, e) in copies.iter().zip([[28.0, 25.0], [12.0, 35.0], [28.0, 35.0]]) {
            assert!((p[0].x - e[0]).abs() < 1e-9 && (p[0].y - e[1]).abs() < 1e-9);
        }
    }
    #[test]
    fn rotated_axes_clip_on_rectangles_and_accept_half_pixel_centres() {
        for size in [[100.0, 100.0], [101.0, 53.0]] {
            for mode in SymmetryMode::ALL {
                let s = PresetSymmetry::new(mode, [size[0] / 2.0, size[1] / 2.0], 0.0).unwrap();
                let segments = s.segments(size);
                assert_eq!(segments.len(), usize::from(s.axes));
                for [a, b] in segments {
                    for p in [a, b] {
                        assert!(p[0] >= -1e-9 && p[0] <= size[0] + 1e-9 && p[1] >= -1e-9 && p[1] <= size[1] + 1e-9);
                    }
                }
                if mode == SymmetryMode::Diagonal {
                    let d = s.directions()[0];
                    assert!((d[0] - d[1]).abs() < 1e-9);
                }
            }
        }
        let s = PresetSymmetry::new(SymmetryMode::Vertical, [20.0, 30.0], 90.0).unwrap();
        let p = s.reflected_passes(&[StrokePoint::new(12.0, 25.0, 1.0)])[0][0];
        assert!((p.x - 12.0).abs() < 1e-9 && (p.y - 35.0).abs() < 1e-9);
        assert!(PresetSymmetry::new(SymmetryMode::Dual, [f64::NAN, 0.0], 0.0).is_none());
        assert!(PresetSymmetry::new(SymmetryMode::Dual, [0.0, 0.0], f64::INFINITY).is_none());
        assert!(PresetSymmetry::new(SymmetryMode::Dual, [MAX_COORDINATE + 1.0, 0.0], 0.0).is_none());
        assert!(PresetSymmetry::new(SymmetryMode::Horizontal, [20.0, -5.0], 0.0).unwrap().segments([100.0, 60.0]).is_empty());
    }

    fn close(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
    }
    #[test]
    fn five_axis_mandala_paints_the_dihedral_group_and_keeps_stylus_data() {
        let c = [20.0, 30.0];
        let s = PresetSymmetry::new(SymmetryMode::Mandala, c, 0.0).unwrap().with_axes(5).unwrap();
        assert_eq!((s.axes, s.mirror_count()), (5, 9));
        let p = StrokePoint { x: 27.0, y: 21.0, pressure: 0.3, tilt_y: -12.0, ..Default::default() };
        let copies = s.reflected_passes(&[p]);
        assert_eq!(copies.len() + 1, 10, "5 axes give 10 copies including the original");
        let (r, theta) = ((p.x - c[0]).hypot(p.y - c[1]), (p.y - c[1]).atan2(p.x - c[0]));
        let mut positions = vec![[p.x, p.y]];
        for copy in &copies {
            let q = copy[0];
            assert_eq!((q.pressure, q.tilt_y), (p.pressure, p.tilt_y));
            assert!(((q.x - c[0]).hypot(q.y - c[1]) - r).abs() < 1e-9, "copies stay on the circle about the centre");
            assert!(positions.iter().all(|&o| !close(o, [q.x, q.y])), "every copy is distinct");
            positions.push([q.x, q.y]);
        }
        // Reflection across the line at angle a sends theta to 2a - theta; rotations add 72° steps.
        let at = |a: f64| [c[0] + r * a.cos(), c[1] + r * a.sin()];
        for (k, copy) in copies.iter().enumerate().take(5) {
            let a = (90.0 + 36.0 * k as f64).to_radians();
            assert!(close([copy[0].x, copy[0].y], at(2.0 * a - theta)), "reflection {k}");
        }
        for (k, copy) in copies.iter().skip(5).enumerate() {
            assert!(close([copy[0].x, copy[0].y], at(theta + (72.0 * (k + 1) as f64).to_radians())), "rotation {}", k + 1);
        }
        let two = PresetSymmetry::new(SymmetryMode::Mandala, c, 0.0).unwrap().with_axes(2).unwrap().reflected_passes(&[p]);
        let dual = PresetSymmetry::new(SymmetryMode::Dual, c, 0.0).unwrap().reflected_passes(&[p]);
        for (a, b) in two.iter().zip(&dual) {
            assert!(close([a[0].x, a[0].y], [b[0].x, b[0].y]), "a two-line mandala is the dual-axis preset");
        }
    }
    #[test]
    fn mandala_axes_are_validated_and_rotation_turns_every_line() {
        let base = PresetSymmetry::new(SymmetryMode::Mandala, [50.0, 40.0], 0.0).unwrap();
        assert_eq!(base.axes, DEFAULT_AXES);
        for bad in [0, 1, MAX_AXES + 1, u8::MAX] {
            assert!(base.with_axes(bad).is_none());
        }
        for n in MIN_AXES..=12 {
            let s = base.with_axes(n).unwrap();
            assert_eq!((s.directions().len(), s.reflected_passes(&[StrokePoint::new(1.0, 2.0, 1.0)]).len()), (usize::from(n), 2 * usize::from(n) - 1));
        }
        assert_eq!(PresetSymmetry::new(SymmetryMode::Vertical, [0.0, 0.0], 0.0).unwrap().with_axes(9).unwrap().axes, 1);
        let turned = PresetSymmetry::new(SymmetryMode::Mandala, [50.0, 40.0], 10.0).unwrap();
        for (a, b) in base.directions().into_iter().zip(turned.directions()) {
            let angle = (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]).to_degrees();
            assert!((angle - 10.0).abs() < 1e-9, "rotation turns each line by the same angle: {angle}");
        }
        for (k, d) in turned.directions().into_iter().enumerate() {
            let expected = (100.0 + 36.0 * k as f64).to_radians();
            assert!(close(d, [expected.cos(), expected.sin()]));
        }
    }
    #[test]
    fn rotated_mandala_lines_clip_to_the_document_through_the_centre() {
        let size = [120.0, 70.0];
        for rotation in [0.0, 17.0, 33.3, 90.0, 271.0] {
            let s = PresetSymmetry::new(SymmetryMode::Mandala, [60.0, 35.0], rotation).unwrap().with_axes(5).unwrap();
            let segments = s.segments(size);
            assert_eq!(segments.len(), 5);
            for ([a, b], d) in segments.into_iter().zip(s.directions()) {
                let on_edge = |p: [f64; 2]| p[0].abs() < 1e-6 || (p[0] - size[0]).abs() < 1e-6 || p[1].abs() < 1e-6 || (p[1] - size[1]).abs() < 1e-6;
                assert!(on_edge(a) && on_edge(b), "lines run edge to edge: {a:?} {b:?}");
                for p in [a, b] {
                    assert!(p[0] >= -1e-9 && p[0] <= size[0] + 1e-9 && p[1] >= -1e-9 && p[1] <= size[1] + 1e-9);
                    let [dx, dy] = [p[0] - 60.0, p[1] - 35.0];
                    assert!((dx * d[1] - dy * d[0]).abs() < 1e-6, "endpoints lie on the rotated line");
                }
            }
        }
        let outside = PresetSymmetry::new(SymmetryMode::Mandala, [-500.0, 35.0], 0.0).unwrap().with_axes(5).unwrap();
        assert!(outside.segments(size).len() < 5, "lines missing the document are dropped");
    }
}
