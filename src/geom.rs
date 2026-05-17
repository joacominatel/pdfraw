//! Geometry primitives: points, bounding boxes, and 2D affine matrices.
//!
//! PDF text is positioned through 3×3 affine matrices applied to a unit
//! coordinate. We only need the 6 affine coefficients (a, b, c, d, e, f); the
//! bottom row is always `[0, 0, 1]`.
//!
//! The PDF spec stores coordinates **bottom-up** (origin at the bottom-left).
//! Throughout this crate, *after* extraction we expose top-down coordinates
//! (origin at the top-left) — see [`Matrix::y_flip`] and the per-page
//! transform applied in the parser.

/// A 2D point in PDF user space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// X coordinate.
    pub x: f32,
    /// Y coordinate.
    pub y: f32,
}

impl Point {
    /// Construct a new point.
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBox {
    /// Left edge (smallest x).
    pub x0: f32,
    /// Top edge (smallest top — top-down coords).
    pub top: f32,
    /// Right edge (largest x).
    pub x1: f32,
    /// Bottom edge (largest top — top-down coords).
    pub bottom: f32,
}

impl BBox {
    /// Construct a bounding box from its four edges.
    pub const fn new(x0: f32, top: f32, x1: f32, bottom: f32) -> Self {
        Self {
            x0,
            top,
            x1,
            bottom,
        }
    }

    /// Width of the box.
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    /// Height of the box (bottom - top in top-down coords).
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Merge two bounding boxes into their enclosing box.
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            x0: self.x0.min(other.x0),
            top: self.top.min(other.top),
            x1: self.x1.max(other.x1),
            bottom: self.bottom.max(other.bottom),
        }
    }
}

/// A 2D affine matrix `[a b 0; c d 0; e f 1]` (column-major as used by PDF).
///
/// PDF text positioning operates almost entirely through these matrices: the
/// text matrix (`Tm`) and the current transformation matrix (`cm`) are
/// composed to produce the on-page position of every glyph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    /// `m[0][0]` — x scale / cos(theta).
    pub a: f32,
    /// `m[0][1]` — y shear / sin(theta).
    pub b: f32,
    /// `m[1][0]` — x shear / -sin(theta).
    pub c: f32,
    /// `m[1][1]` — y scale / cos(theta).
    pub d: f32,
    /// `m[2][0]` — x translation.
    pub e: f32,
    /// `m[2][1]` — y translation.
    pub f: f32,
}

impl Matrix {
    /// Identity matrix.
    pub const IDENTITY: Matrix = Matrix {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// Construct from the 6 affine components, in PDF operator order.
    pub const fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// Pure translation.
    pub const fn translation(tx: f32, ty: f32) -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: tx,
            f: ty,
        }
    }

    /// Pure scale.
    pub const fn scale(sx: f32, sy: f32) -> Self {
        Self {
            a: sx,
            b: 0.0,
            c: 0.0,
            d: sy,
            e: 0.0,
            f: 0.0,
        }
    }

    /// Matrix multiplication: `self * other`.
    ///
    /// PDF composes transforms by post-multiplying: a `cm` operator applies
    /// `new_ctm = applied_matrix * current_ctm`. We use the convention that
    /// `self.then(other)` returns `self * other`, i.e. the transform that
    /// first applies `self` and then `other`.
    pub fn then(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }

    /// Apply this matrix to a point.
    pub fn transform(&self, p: Point) -> Point {
        Point {
            x: self.a * p.x + self.c * p.y + self.e,
            y: self.b * p.x + self.d * p.y + self.f,
        }
    }

    /// Translate `self` by `(tx, ty)` in its local frame (pre-multiply).
    ///
    /// Equivalent to `Matrix::translation(tx, ty).then(*self)`.
    pub fn pre_translate(self, tx: f32, ty: f32) -> Self {
        Matrix::translation(tx, ty).then(self)
    }

    /// Approximate horizontal scale of this matrix.
    ///
    /// Used to compute the effective font size when the text matrix is
    /// rotated or skewed.
    pub fn x_scale(&self) -> f32 {
        (self.a * self.a + self.b * self.b).sqrt()
    }

    /// Approximate vertical scale of this matrix.
    pub fn y_scale(&self) -> f32 {
        (self.c * self.c + self.d * self.d).sqrt()
    }

    /// True when the matrix is axis-aligned and not rotated/flipped beyond
    /// a tolerance of 0.01 radians.
    pub fn is_upright(&self) -> bool {
        // For an upright matrix `a > 0, d != 0` and skew/shear coefficients
        // (`b`, `c`) are near zero relative to the main diagonal.
        let scale = self.x_scale().max(self.y_scale()).max(1e-6);
        self.b.abs() / scale < 0.01 && self.c.abs() / scale < 0.01 && self.a > 0.0
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_times_identity_is_identity() {
        let r = Matrix::IDENTITY.then(Matrix::IDENTITY);
        assert_eq!(r, Matrix::IDENTITY);
    }

    #[test]
    fn translation_transforms_point() {
        let m = Matrix::translation(3.0, 5.0);
        assert_eq!(m.transform(Point::new(1.0, 2.0)), Point::new(4.0, 7.0));
    }

    #[test]
    fn scale_transforms_point() {
        let m = Matrix::scale(2.0, 3.0);
        assert_eq!(m.transform(Point::new(4.0, 5.0)), Point::new(8.0, 15.0));
    }

    #[test]
    fn then_applies_self_first_then_other() {
        // First translate(1,2), then scale(2,3): point (1,1) -> (2,3) -> (4,9)
        let m = Matrix::translation(1.0, 2.0).then(Matrix::scale(2.0, 3.0));
        assert_eq!(m.transform(Point::new(1.0, 1.0)), Point::new(4.0, 9.0));
    }

    #[test]
    fn pre_translate_adds_to_translation_column() {
        let m = Matrix::scale(2.0, 2.0).pre_translate(1.0, 0.0);
        // pre-translate means translate first, then scale: (0,0) -> (1,0) -> (2,0)
        assert_eq!(m.transform(Point::new(0.0, 0.0)), Point::new(2.0, 0.0));
    }

    #[test]
    fn upright_identity_is_upright() {
        assert!(Matrix::IDENTITY.is_upright());
    }

    #[test]
    fn rotated_matrix_is_not_upright() {
        // 45° rotation
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let m = Matrix::new(s, s, -s, s, 0.0, 0.0);
        assert!(!m.is_upright());
    }

    #[test]
    fn x_scale_returns_horizontal_factor_when_matrix_is_pure_scale() {
        let m = Matrix::scale(3.0, 5.0);
        assert!((m.x_scale() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn bbox_merge_returns_enclosing_box() {
        let a = BBox::new(0.0, 0.0, 5.0, 5.0);
        let b = BBox::new(3.0, 3.0, 10.0, 10.0);
        assert_eq!(a.merge(&b), BBox::new(0.0, 0.0, 10.0, 10.0));
    }
}
