/// Axis-aligned rectangle in floating-point coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.w / 2.0
    }

    pub fn center_y(&self) -> f32 {
        self.y + self.h / 2.0
    }

    pub fn center(&self) -> Point {
        Point::new(self.center_x(), self.center_y())
    }
}

/// A 2D point in floating-point coordinates.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_new_and_accessors() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.x, 10.0);
        assert_eq!(r.y, 20.0);
        assert_eq!(r.w, 100.0);
        assert_eq!(r.h, 50.0);
    }

    #[test]
    fn rect_bottom() {
        let r = Rect::new(5.0, 10.0, 30.0, 40.0);
        assert_eq!(r.bottom(), 50.0);
    }

    #[test]
    fn rect_center() {
        let r = Rect::new(0.0, 0.0, 100.0, 200.0);
        assert_eq!(r.center_x(), 50.0);
        assert_eq!(r.center_y(), 100.0);
        let c = r.center();
        assert_eq!(c.x, 50.0);
        assert_eq!(c.y, 100.0);
    }

    #[test]
    fn point_new() {
        let p = Point::new(3.14, 2.72);
        assert_eq!(p.x, 3.14);
        assert_eq!(p.y, 2.72);
    }

    #[test]
    fn rect_zero_size_bottom() {
        let r = Rect::new(5.0, 5.0, 0.0, 0.0);
        assert_eq!(r.bottom(), 5.0);
    }
}
