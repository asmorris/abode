//! 2D geometry in metres. Plan coordinates: +x is right, +y is down (like the screen).

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

const EPS: f64 = 1e-9;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn length(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn distance(self, o: Vec2) -> f64 {
        (self - o).length()
    }
    pub fn dot(self, o: Vec2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: Vec2) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn normalized(self) -> Vec2 {
        let l = self.length();
        if l > EPS {
            self / l
        } else {
            Vec2::ZERO
        }
    }
    /// Rotated 90°.
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }
    pub fn rotated(self, angle: f64) -> Vec2 {
        let (s, c) = angle.sin_cos();
        Vec2::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }
    pub fn lerp(self, o: Vec2, t: f64) -> Vec2 {
        self + (o - self) * t
    }
    pub fn min(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.min(o.x), self.y.min(o.y))
    }
    pub fn max(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x.max(o.x), self.y.max(o.y))
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        *self = *self + o;
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        *self = *self - o;
    }
}
impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f64) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl Div<f64> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f64) -> Vec2 {
        Vec2::new(self.x / s, self.y / s)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// Shoelace signed area. Positive for one winding, negative for the other.
pub fn signed_area(poly: &[Vec2]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    (0..n).map(|i| poly[i].cross(poly[(i + 1) % n])).sum::<f64>() / 2.0
}

pub fn area(poly: &[Vec2]) -> f64 {
    signed_area(poly).abs()
}

pub fn perimeter(poly: &[Vec2]) -> f64 {
    let n = poly.len();
    (0..n).map(|i| poly[i].distance(poly[(i + 1) % n])).sum()
}

pub fn centroid(poly: &[Vec2]) -> Vec2 {
    let n = poly.len();
    if n == 0 {
        return Vec2::ZERO;
    }
    let a = signed_area(poly);
    if a.abs() < EPS {
        let sum = poly.iter().fold(Vec2::ZERO, |acc, p| acc + *p);
        return sum / n as f64;
    }
    let mut c = Vec2::ZERO;
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let f = p.cross(q);
        c += (p + q) * f;
    }
    c / (6.0 * a)
}

/// Even-odd ray casting.
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) {
            let x = (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x;
            if p.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Closest point on segment `a..b` and its parameter `t` in `0..=1`.
pub fn closest_point_on_segment(p: Vec2, a: Vec2, b: Vec2) -> (Vec2, f64) {
    let ab = b - a;
    let len2 = ab.dot(ab);
    if len2 < EPS {
        return (a, 0.0);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (a + ab * t, t)
}

pub fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    closest_point_on_segment(p, a, b).0.distance(p)
}

pub fn bounds<'a>(points: impl IntoIterator<Item = &'a Vec2>) -> Option<(Vec2, Vec2)> {
    let mut it = points.into_iter();
    let first = *it.next()?;
    Some(it.fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))))
}

fn orient(a: Vec2, b: Vec2, c: Vec2) -> f64 {
    (b - a).cross(c - a)
}

/// True if the segments cross at a single interior point.
pub fn segments_cross(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> bool {
    let e = 1e-12;
    let d1 = orient(p3, p4, p1);
    let d2 = orient(p3, p4, p2);
    let d3 = orient(p1, p2, p3);
    let d4 = orient(p1, p2, p4);
    ((d1 > e && d2 < -e) || (d1 < -e && d2 > e)) && ((d3 > e && d4 < -e) || (d3 < -e && d4 > e))
}

/// A polygon is simple if no two non-adjacent edges cross.
pub fn is_simple(poly: &[Vec2]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        for j in (i + 1)..n {
            if j == i || (j + 1) % n == i || (i + 1) % n == j {
                continue;
            }
            let (c, d) = (poly[j], poly[(j + 1) % n]);
            if segments_cross(a, b, c, d) {
                return false;
            }
        }
    }
    true
}

fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    let e = 1e-12;
    orient(a, b, p) > e && orient(b, c, p) > e && orient(c, a, p) > e
}

/// Ear-clipping triangulation for simple polygons (convex or concave).
/// Returns triangles as indices into `poly`.
pub fn triangulate(poly: &[Vec2]) -> Vec<[usize; 3]> {
    let n = poly.len();
    if n < 3 {
        return Vec::new();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if signed_area(poly) < 0.0 {
        idx.reverse();
    }
    let mut tris = Vec::with_capacity(n - 2);
    let mut guard = 0;
    while idx.len() > 3 && guard < n * n + 10 {
        guard += 1;
        let m = idx.len();
        let mut clipped = false;
        for i in 0..m {
            let (ia, ib, ic) = (idx[(i + m - 1) % m], idx[i], idx[(i + 1) % m]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if orient(a, b, c) <= 1e-12 {
                continue; // reflex or collinear
            }
            let blocked = idx
                .iter()
                .any(|&j| j != ia && j != ib && j != ic && point_in_triangle(poly[j], a, b, c));
            if blocked {
                continue;
            }
            tris.push([ia, ib, ic]);
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            // Only collinear/degenerate vertices left to clip: drop one and retry.
            let m = idx.len();
            let degenerate = (0..m).find(|&i| {
                let (a, b, c) = (poly[idx[(i + m - 1) % m]], poly[idx[i]], poly[idx[(i + 1) % m]]);
                orient(a, b, c).abs() <= 1e-12
            });
            match degenerate {
                Some(i) => {
                    idx.remove(i);
                }
                None => break,
            }
        }
    }
    if idx.len() == 3 && orient(poly[idx[0]], poly[idx[1]], poly[idx[2]]).abs() > 1e-12 {
        tris.push([idx[0], idx[1], idx[2]]);
    }
    tris
}

/// A good point for a label: the centroid, or (for odd shapes where that falls
/// outside) the centre of the largest triangle.
pub fn label_point(poly: &[Vec2]) -> Vec2 {
    let c = centroid(poly);
    if point_in_polygon(c, poly) {
        return c;
    }
    triangulate(poly)
        .into_iter()
        .map(|[a, b, c]| (poly[a], poly[b], poly[c]))
        .max_by(|x, y| tri_area(*x).total_cmp(&tri_area(*y)))
        .map(|(a, b, c)| (a + b + c) / 3.0)
        .unwrap_or(c)
}

fn tri_area((a, b, c): (Vec2, Vec2, Vec2)) -> f64 {
    orient(a, b, c).abs() / 2.0
}

pub fn rect_points(min: Vec2, width: f64, depth: f64) -> Vec<Vec2> {
    vec![
        min,
        Vec2::new(min.x + width, min.y),
        Vec2::new(min.x + width, min.y + depth),
        Vec2::new(min.x, min.y + depth),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l_shape() -> Vec<Vec2> {
        vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(4.0, 0.0),
            Vec2::new(4.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, 4.0),
            Vec2::new(0.0, 4.0),
        ]
    }

    #[test]
    fn area_and_perimeter() {
        let sq = rect_points(Vec2::ZERO, 3.0, 2.0);
        assert!((area(&sq) - 6.0).abs() < 1e-9);
        assert!((perimeter(&sq) - 10.0).abs() < 1e-9);
        assert!((area(&l_shape()) - 7.0).abs() < 1e-9);
    }

    #[test]
    fn point_in_poly() {
        let l = l_shape();
        assert!(point_in_polygon(Vec2::new(0.5, 3.0), &l));
        assert!(!point_in_polygon(Vec2::new(3.0, 3.0), &l));
    }

    #[test]
    fn triangulates_concave() {
        let l = l_shape();
        let tris = triangulate(&l);
        assert_eq!(tris.len(), 4);
        let total: f64 = tris.iter().map(|t| tri_area((l[t[0]], l[t[1]], l[t[2]]))).sum();
        assert!((total - 7.0).abs() < 1e-9);
    }

    #[test]
    fn triangulates_with_collinear_points() {
        // A rectangle with an extra point in the middle of the top edge (a T-junction).
        let mut r = rect_points(Vec2::ZERO, 4.0, 2.0);
        r.insert(1, Vec2::new(2.0, 0.0));
        let tris = triangulate(&r);
        let total: f64 = tris.iter().map(|t| tri_area((r[t[0]], r[t[1]], r[t[2]]))).sum();
        assert!((total - 8.0).abs() < 1e-9);
    }

    #[test]
    fn label_point_inside_l() {
        let l = l_shape();
        assert!(point_in_polygon(label_point(&l), &l));
    }

    #[test]
    fn simple_polygon_detection() {
        assert!(is_simple(&l_shape()));
        let bowtie = vec![
            Vec2::new(0.0, 0.0),
            Vec2::new(2.0, 2.0),
            Vec2::new(2.0, 0.0),
            Vec2::new(0.0, 2.0),
        ];
        assert!(!is_simple(&bowtie));
    }
}
