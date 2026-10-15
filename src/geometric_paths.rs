// Original geometric paths; matches assets/cursors/geometric.json.
#[derive(Clone, Copy)]
pub enum GeometricCursor {
    Text,
    VerticalText,
    Link,
    Grip,
    OpenHand,
    HorizontalLeft,
    HorizontalRight,
    HorizontalBoth,
}
enum Segment {
    Move(f64, f64),
    Line(f64, f64),
    Curve(f64, f64, f64, f64, f64, f64),
    Close,
}
const TEXT: &[Segment] = &[
    Segment::Move(24.0, 16.0),
    Segment::Line(40.0, 16.0),
    Segment::Move(32.0, 16.0),
    Segment::Line(32.0, 48.0),
    Segment::Move(24.0, 48.0),
    Segment::Line(40.0, 48.0),
];
const LINK: &[Segment] = &[
    Segment::Move(24.0, 35.0),
    Segment::Line(24.0, 18.0),
    Segment::Curve(24.0, 14.0, 30.0, 14.0, 30.0, 18.0),
    Segment::Line(30.0, 29.0),
    Segment::Line(41.0, 29.0),
    Segment::Curve(45.0, 29.0, 48.0, 32.0, 48.0, 36.0),
    Segment::Line(48.0, 39.0),
    Segment::Curve(48.0, 46.0, 43.0, 50.0, 36.0, 50.0),
    Segment::Line(32.0, 50.0),
    Segment::Curve(29.0, 50.0, 27.0, 49.0, 25.0, 47.0),
    Segment::Line(17.0, 38.0),
    Segment::Curve(15.0, 36.0, 15.0, 34.0, 17.0, 33.0),
    Segment::Curve(18.0, 32.0, 19.0, 32.0, 21.0, 34.0),
    Segment::Line(24.0, 37.0),
    Segment::Close,
];
const GRIP: &[Segment] = &[
    Segment::Move(21.0, 31.0),
    Segment::Line(21.0, 26.0),
    Segment::Curve(21.0, 23.0, 23.0, 21.0, 26.0, 21.0),
    Segment::Line(39.0, 21.0),
    Segment::Curve(44.0, 21.0, 47.0, 24.0, 47.0, 29.0),
    Segment::Line(47.0, 38.0),
    Segment::Curve(47.0, 44.0, 42.0, 49.0, 36.0, 49.0),
    Segment::Line(31.0, 49.0),
    Segment::Curve(27.0, 49.0, 24.0, 47.0, 22.0, 44.0),
    Segment::Line(17.0, 37.0),
    Segment::Curve(15.0, 34.0, 17.0, 31.0, 20.0, 33.0),
    Segment::Line(25.0, 37.0),
    Segment::Close,
];
const HORIZONTAL: &[Segment] = &[
    Segment::Move(25.0, 23.0),
    Segment::Line(16.0, 32.0),
    Segment::Line(25.0, 41.0),
    Segment::Move(39.0, 23.0),
    Segment::Line(48.0, 32.0),
    Segment::Line(39.0, 41.0),
    Segment::Move(17.0, 32.0),
    Segment::Line(47.0, 32.0),
];
const OPEN_HAND: &[Segment] = &[
    Segment::Move(21.0, 34.0),
    Segment::Line(21.0, 22.0),
    Segment::Curve(21.0, 18.0, 27.0, 18.0, 27.0, 22.0),
    Segment::Line(27.0, 17.0),
    Segment::Curve(27.0, 13.0, 33.0, 13.0, 33.0, 17.0),
    Segment::Line(33.0, 20.0),
    Segment::Curve(33.0, 16.0, 39.0, 16.0, 39.0, 20.0),
    Segment::Line(39.0, 24.0),
    Segment::Curve(39.0, 20.0, 45.0, 20.0, 45.0, 24.0),
    Segment::Line(45.0, 37.0),
    Segment::Curve(45.0, 45.0, 40.0, 49.0, 33.0, 49.0),
    Segment::Curve(28.0, 49.0, 25.0, 47.0, 22.0, 43.0),
    Segment::Line(16.0, 34.0),
    Segment::Curve(14.0, 31.0, 18.0, 29.0, 20.0, 32.0),
    Segment::Line(24.0, 37.0),
    Segment::Close,
];
const GRIP_DETAILS: &[Segment] = &[
    Segment::Move(29.0, 22.0),
    Segment::Line(29.0, 29.0),
    Segment::Move(37.0, 22.0),
    Segment::Line(37.0, 29.0),
];
impl GeometricCursor {
    fn path(self) -> &'static [Segment] {
        match self {
            Self::Text | Self::VerticalText => TEXT,
            Self::Link => LINK,
            Self::Grip => GRIP,
            Self::OpenHand => OPEN_HAND,
            _ => HORIZONTAL,
        }
    }
    fn filled(self) -> bool {
        matches!(self, Self::Link | Self::Grip | Self::OpenHand)
    }
    pub fn hotspot(self) -> Point {
        match self {
            Self::Link => Point::new(27., 16.),
            _ => Point::new(32., 32.),
        }
    }
}
