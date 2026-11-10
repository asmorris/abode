//! Room outlines with curved sides, and cutting one out of another.
//!
//! An outline is a loop of corners, each with the bulge of the side that
//! leaves it (0 for straight; see [`crate::geometry::arc_length`]). When a
//! new room is drawn over an existing one, [`subtract`] trims the new room
//! back to the existing room's walls, curves included, so the two can share
//! them.

use crate::geometry::*;

/// Corners in order, each with the bulge of the side to the next corner.
pub type Outline = Vec<(Vec2, f64)>;

const EPS: f64 = 1e-7;

#[derive(Clone, Copy, Debug)]
struct Edge {
    a: Vec2,
    b: Vec2,
    bulge: f64,
}

impl Edge {
    fn curved(self) -> bool {
        self.bulge.abs() > 1e-9 && self.a.distance(self.b) > EPS
    }

    fn len(self) -> f64 {
        arc_length(self.a, self.b, self.bulge)
    }

    /// Point `t` of the way along (by length).
    fn at(self, t: f64) -> Vec2 {
        if !self.curved() {
            return self.a.lerp(self.b, t);
        }
        arc_point(self.a, self.b, self.bulge, t * self.len()).0
    }

    fn dir_at(self, t: f64) -> Vec2 {
        if !self.curved() {
            return (self.b - self.a).normalized();
        }
        arc_point(self.a, self.b, self.bulge, t * self.len()).1
    }

    /// The part from `t0` to `t1`.
    fn sub(self, t0: f64, t1: f64) -> Edge {
        Edge { a: self.at(t0), b: self.at(t1), bulge: split_bulge(self.bulge, t1 - t0) }
    }

    fn reversed(self) -> Edge {
        Edge { a: self.b, b: self.a, bulge: -self.bulge }
    }

    /// (centre, radius, swept angle) of a curved side.
    fn circle(self) -> (Vec2, f64, f64) {
        let (c, sweep) = arc_center(self.a, self.b, self.bulge);
        (c, c.distance(self.a), sweep)
    }

    /// How far along a point already known to be on the circle is, if it's
    /// within the arc.
    fn arc_param(self, p: Vec2) -> Option<f64> {
        let (c, _, sweep) = self.circle();
        let (va, vp) = (self.a - c, p - c);
        let mut phi = va.cross(vp).atan2(va.dot(vp));
        if sweep > 0.0 && phi < -EPS {
            phi += std::f64::consts::TAU;
        } else if sweep < 0.0 && phi > EPS {
            phi -= std::f64::consts::TAU;
        }
        let t = phi / sweep;
        (-EPS..=1.0 + EPS).contains(&t).then_some(t.clamp(0.0, 1.0))
    }

    /// How far along `p` is, if it's on this side (within `tol`).
    fn param_of(self, p: Vec2, tol: f64) -> Option<f64> {
        if !self.curved() {
            let (q, t) = closest_point_on_segment(p, self.a, self.b);
            return (q.distance(p) <= tol).then_some(t);
        }
        let (c, r, _) = self.circle();
        if (p.distance(c) - r).abs() > tol {
            return None;
        }
        self.arc_param(c + (p - c).normalized() * r)
    }

    fn distance(self, p: Vec2) -> f64 {
        if !self.curved() {
            return distance_to_segment(p, self.a, self.b);
        }
        let (c, r, _) = self.circle();
        match self.arc_param(c + (p - c).normalized() * r) {
            Some(_) => (p.distance(c) - r).abs(),
            None => p.distance(self.a).min(p.distance(self.b)),
        }
    }
}

/// Bulge of a piece `fraction` of the way along an arc of bulge `bulge`.
pub fn split_bulge(bulge: f64, fraction: f64) -> f64 {
    (bulge.atan() * fraction).tan()
}

fn edges(o: &[(Vec2, f64)]) -> Vec<Edge> {
    let n = o.len();
    (0..n).map(|i| Edge { a: o[i].0, b: o[(i + 1) % n].0, bulge: o[i].1 }).collect()
}

/// The outline as points, curves broken into short straight pieces.
pub fn tessellate(o: &[(Vec2, f64)]) -> Vec<Vec2> {
    let mut out = Vec::new();
    for e in edges(o) {
        let pts = arc_polyline(e.a, e.b, e.bulge);
        out.extend_from_slice(&pts[..pts.len() - 1]);
    }
    out
}

/// Area (positive when the corners go anticlockwise, in maths terms).
pub fn signed_area_of(o: &[(Vec2, f64)]) -> f64 {
    signed_area(&tessellate(o))
}

/// The same outline going the other way round.
pub fn reverse(o: &[(Vec2, f64)]) -> Outline {
    let n = o.len();
    (0..n).map(|j| (o[n - 1 - j].0, -o[(2 * n - 2 - j) % n].1)).collect()
}

/// Is `p` inside? Exact for curved sides: the straight-sided polygon, with
/// the sliver between each curve and its chord added or taken away.
pub fn contains(o: &[(Vec2, f64)], p: Vec2) -> bool {
    let corners: Vec<Vec2> = o.iter().map(|c| c.0).collect();
    let mut inside = point_in_polygon(p, &corners);
    for e in edges(o).into_iter().filter(|e| e.curved()) {
        let (c, r, _) = e.circle();
        // The sliver is the part of the disc on the curve's side of the chord.
        let toward = (e.b - e.a).perp() * e.bulge.signum();
        if p.distance(c) < r && (p - e.a).dot(toward) > 0.0 {
            inside = !inside;
        }
    }
    inside
}

/// Where two sides cross: (how far along `e`, how far along `f`).
fn crossings(e: Edge, f: Edge) -> Vec<(f64, f64)> {
    let mut pts: Vec<Vec2> = Vec::new();
    match (e.curved(), f.curved()) {
        (false, false) => {
            let (r, s) = (e.b - e.a, f.b - f.a);
            let d = r.cross(s);
            if d.abs() > 1e-12 {
                let t = (f.a - e.a).cross(s) / d;
                pts.push(e.a + r * t);
            }
        }
        (true, false) | (false, true) => {
            let (arc, line) = if e.curved() { (e, f) } else { (f, e) };
            let (c, r, _) = arc.circle();
            let d = line.b - line.a;
            let m = line.a - c;
            let (qa, qb, qc) = (d.dot(d), 2.0 * m.dot(d), m.dot(m) - r * r);
            let disc = qb * qb - 4.0 * qa * qc;
            if qa > 1e-18 && disc >= 0.0 {
                let sq = disc.sqrt();
                for t in [(-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa)] {
                    pts.push(line.a + d * t);
                }
            }
        }
        (true, true) => {
            let ((c1, r1, _), (c2, r2, _)) = (e.circle(), f.circle());
            let d = c1.distance(c2);
            if d > 1e-12 && d <= r1 + r2 && d >= (r1 - r2).abs() {
                let a = (r1 * r1 - r2 * r2 + d * d) / (2.0 * d);
                let h = (r1 * r1 - a * a).max(0.0).sqrt();
                let u = (c2 - c1) / d;
                let base = c1 + u * a;
                pts.push(base + u.perp() * h);
                pts.push(base - u.perp() * h);
            }
        }
    }
    let tol = 1e-6;
    pts.into_iter().filter_map(|p| Some((e.param_of(p, tol)?, f.param_of(p, tol)?))).collect()
}

#[derive(Clone, Copy)]
struct Piece {
    edge: Edge,
    /// Which outline (0 = the one being cut, 1 = the cutter), which side,
    /// and the stretch of it, so neighbouring bits of one side can be rejoined.
    from: (usize, usize, f64, f64),
}

fn pieces(es: &[Edge], cuts: &[Vec<f64>], which: usize) -> Vec<Piece> {
    let mut out = Vec::new();
    for (i, e) in es.iter().enumerate() {
        let mut ts: Vec<f64> = cuts[i].iter().copied().filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6).collect();
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        let mut prev = 0.0;
        for t in ts.into_iter().chain([1.0]) {
            out.push(Piece { edge: e.sub(prev, t), from: (which, i, prev, t) });
            prev = t;
        }
    }
    out
}

/// Why an outline can't be cut out of another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutError {
    /// Nothing would be left.
    Covered,
    /// The cutter is inside, which would leave a hole.
    Hole,
}

/// `a` with `b` cut out of it. Both should go anticlockwise (positive
/// area). Sides of `a` that run along `b` are kept, so the two can share
/// them. If `b` splits `a` in two, every piece is returned, biggest first.
pub fn subtract(a: &[(Vec2, f64)], b: &[(Vec2, f64)]) -> Result<Vec<Outline>, CutError> {
    let (ea, eb) = (edges(a), edges(b));
    let mut cut_a: Vec<Vec<f64>> = vec![Vec::new(); ea.len()];
    let mut cut_b: Vec<Vec<f64>> = vec![Vec::new(); eb.len()];
    for (i, e) in ea.iter().enumerate() {
        for (j, f) in eb.iter().enumerate() {
            for (t, u) in crossings(*e, *f) {
                cut_a[i].push(t);
                cut_b[j].push(u);
            }
        }
    }
    // Corners of one lying on a side of the other (where sides run together).
    let tol = 1e-6;
    for (i, e) in ea.iter().enumerate() {
        for f in &eb {
            if let Some(t) = e.param_of(f.a, tol) {
                cut_a[i].push(t);
            }
        }
    }
    for (j, f) in eb.iter().enumerate() {
        for e in &ea {
            if let Some(u) = f.param_of(e.a, tol) {
                cut_b[j].push(u);
            }
        }
    }

    let probe = 1e-4;
    let inward = |e: Edge, t: f64| e.at(t) + e.dir_at(t).perp() * probe;
    let outward = |e: Edge, t: f64| e.at(t) - e.dir_at(t).perp() * probe;
    let mut keep: Vec<Piece> = Vec::new();
    // Bits of `a` not inside `b` (including along `b`'s outside).
    for p in pieces(&ea, &cut_a, 0) {
        if !contains(b, inward(p.edge, 0.5)) {
            keep.push(p);
        }
    }
    // Bits of `b` inside `a`, going the other way round.
    for p in pieces(&eb, &cut_b, 1) {
        if contains(a, inward(p.edge, 0.5)) && contains(a, outward(p.edge, 0.5)) && ea.iter().all(|e| e.distance(p.edge.at(0.5)) > tol) {
            keep.push(Piece { edge: p.edge.reversed(), from: p.from });
        }
    }
    if keep.is_empty() {
        return Err(CutError::Covered);
    }

    // Chain the bits into loops.
    let mut used = vec![false; keep.len()];
    let mut loops: Vec<Vec<Piece>> = Vec::new();
    for start in 0..keep.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut chain = vec![keep[start]];
        loop {
            let end = chain.last().unwrap().edge.b;
            if end.distance(chain[0].edge.a) < 1e-5 && chain.len() > 1 {
                break;
            }
            let next = (0..keep.len()).filter(|&k| !used[k]).min_by(|&x, &y| {
                keep[x].edge.a.distance(end).total_cmp(&keep[y].edge.a.distance(end))
            });
            match next {
                Some(k) if keep[k].edge.a.distance(end) < 1e-5 => {
                    used[k] = true;
                    chain.push(keep[k]);
                }
                _ => break,
            }
        }
        loops.push(chain);
    }

    let mut out: Vec<Outline> = Vec::new();
    for chain in loops {
        // Rejoin neighbouring bits of the same side.
        let mut merged: Vec<Piece> = Vec::new();
        for p in chain {
            if let Some(last) = merged.last_mut() {
                let (w, i, t0, t1) = last.from;
                if w == p.from.0 && i == p.from.1 && (t1 - p.from.2).abs() < 1e-9 && w == 0 {
                    let side = ea[i];
                    *last = Piece { edge: side.sub(t0, p.from.3), from: (w, i, t0, p.from.3) };
                    continue;
                }
            }
            merged.push(p);
        }
        if merged.len() > 1 {
            // The loop started partway along a side: join the side's two bits.
            let (first, last) = (merged[0].from, merged.last().unwrap().from);
            if first.0 == 0 && last.0 == 0 && first.1 == last.1 && (last.3 - first.2).abs() < 1e-9 {
                let side = ea[first.1];
                merged.pop();
                merged[0] = Piece { edge: side.sub(last.2, first.3), from: (0, first.1, last.2, first.3) };
            }
        }
        let o: Outline = merged.iter().map(|p| (p.edge.a, p.edge.bulge)).collect();
        if o.len() >= 2 {
            out.push(o);
        }
    }
    if out.iter().any(|o| signed_area_of(o) < 0.0) {
        return Err(CutError::Hole);
    }
    out.retain(|o| signed_area_of(o) > 1e-6);
    if out.is_empty() {
        return Err(CutError::Covered);
    }
    out.sort_by(|x, y| signed_area_of(y).total_cmp(&signed_area_of(x)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Outline {
        // Anticlockwise in maths terms.
        vec![(Vec2::new(x, y), 0.0), (Vec2::new(x + w, y), 0.0), (Vec2::new(x + w, y + h), 0.0), (Vec2::new(x, y + h), 0.0)]
    }

    fn circle(c: Vec2, r: f64) -> Outline {
        let q = (std::f64::consts::FRAC_PI_2 / 4.0).tan();
        let mut o: Outline = [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)].iter().map(|(x, y)| (c + Vec2::new(*x, *y) * r, 0.0)).collect();
        if signed_area_of(&o) < 0.0 {
            o = reverse(&o);
        }
        // Positive bulges curve inwards on an anticlockwise loop.
        for c in &mut o {
            c.1 = -q;
        }
        o
    }

    #[test]
    fn circle_is_round() {
        let o = circle(Vec2::ZERO, 2.0);
        assert!((signed_area_of(&o) - std::f64::consts::PI * 4.0).abs() < 0.05);
        assert!(contains(&o, Vec2::new(0.0, 1.95)));
        assert!(!contains(&o, Vec2::new(1.45, 1.45)));
        assert!(contains(&o, Vec2::new(1.4, 1.4)));
    }

    #[test]
    fn reverse_keeps_curves() {
        let o = circle(Vec2::ZERO, 2.0);
        let r = reverse(&o);
        assert!((signed_area_of(&r) + signed_area_of(&o)).abs() < 1e-9);
    }

    #[test]
    fn circle_over_a_rectangle_gets_a_flat_side() {
        // A circle of radius 2 at (0, 0) over a rectangle whose top is y = 1.
        let c = circle(Vec2::ZERO, 2.0);
        let r = rect(-3.0, 1.0, 6.0, 4.0);
        let out = subtract(&c, &r).unwrap();
        assert_eq!(out.len(), 1);
        let o = &out[0];
        // Everything is above the rectangle's top...
        assert!(tessellate(o).iter().all(|p| p.y <= 1.0 + 1e-9));
        // ...and there's a straight side along it, 2·√3 long.
        let flat = edges(o).into_iter().find(|e| !e.curved() && (e.a.y - 1.0).abs() < 1e-9 && (e.b.y - 1.0).abs() < 1e-9).unwrap();
        assert!((flat.len() - 2.0 * 3f64.sqrt()).abs() < 1e-6);
        // The rest stays on the circle.
        assert!(tessellate(o).iter().filter(|p| p.y < 0.99).all(|p| (p.length() - 2.0).abs() < 1e-6));
        let segment = 4.0 * (0.5f64).acos() - 3f64.sqrt(); // circle area below y = 1
        assert!((signed_area_of(o) - (std::f64::consts::PI * 4.0 - segment)).abs() < 0.05);
    }

    #[test]
    fn rectangle_over_a_circle_gets_a_curved_bite() {
        let c = circle(Vec2::ZERO, 2.0);
        let r = rect(-3.0, 1.0, 6.0, 4.0);
        let out = subtract(&r, &c).unwrap();
        assert_eq!(out.len(), 1);
        assert!(edges(&out[0]).iter().any(|e| e.curved()), "the bite follows the circle");
        assert!(tessellate(&out[0]).iter().all(|p| !contains(&c, *p + Vec2::new(0.0, 1e-3)) || p.y >= 1.0));
    }

    #[test]
    fn touching_rooms_are_left_alone() {
        let a = rect(0.0, 0.0, 2.0, 2.0);
        let b = rect(2.0, 0.0, 2.0, 2.0);
        let out = subtract(&a, &b).unwrap();
        assert!((signed_area_of(&out[0]) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn overlapping_rectangles() {
        let a = rect(0.0, 0.0, 4.0, 2.0);
        let b = rect(3.0, -1.0, 4.0, 2.0);
        let out = subtract(&a, &b).unwrap();
        assert!((signed_area_of(&out[0]) - 7.0).abs() < 1e-9);
        assert_eq!(out[0].len(), 6);
    }

    #[test]
    fn holes_and_covering_are_refused() {
        let big = rect(0.0, 0.0, 10.0, 10.0);
        let small = rect(4.0, 4.0, 2.0, 2.0);
        assert_eq!(subtract(&big, &small), Err(CutError::Hole));
        assert_eq!(subtract(&small, &big), Err(CutError::Covered));
    }
}
