//! Painting a laid-out diagram onto a canvas.
//!
//! Only the geometry is painted here: boxes, frames, connectors and their
//! decorations. Every piece of text is a real view placed by [`crate::label`],
//! so the diagram keeps the platform's own text rendering and shows up in the
//! accessibility tree as words rather than as one opaque picture.
//!
//! The canvas closure runs once, so the geometry is recorded once. The palette
//! is the one thing that moves — a light/dark switch or a custom accent — and
//! each colour is a live operand of the commands that paint with it: the theme
//! changing updates those operands in place and records nothing again.
//!
//! Everything is drawn at the diagram's natural size, in the diagram's own
//! coordinates. Scaling the geometry to fit a container would break the one
//! guarantee this crate is built on — that the box layout reserved for a label
//! is the box its glyphs land in — because the glyphs would still be drawn at
//! their own size. A diagram that does not fit is the container's problem to
//! solve, by scrolling.

use alloc::vec::Vec;

use nami::{Computed, SignalExt as _};
use waterui_core::layout::{Point, Rect};
use waterui_graphics::canvas::{Circle, Cx, Line, Path, PathBuilder, StrokeStyle};
use waterui_graphics::color::WorkingColor;

use crate::layout::{DiagramLayout, Edge, EdgeMarker, EdgeStroke};
use crate::shape;
use crate::theme::Palette;

/// Stroke width of a node outline and an ordinary edge.
const STROKE: f32 = 1.0;
/// Stroke width of a `==>` edge.
const THICK_STROKE: f32 = 2.5;
/// Dash pattern of a `-.->` edge.
const DASH: [f32; 2] = [6.0, 4.0];
/// Length of an arrowhead along the edge.
const ARROW_LENGTH: f32 = 10.0;
/// Half-width of an arrowhead across the edge.
const ARROW_HALF_WIDTH: f32 = 4.0;
/// Radius of a circle edge marker.
const MARKER_RADIUS: f32 = 4.0;
/// Half-diagonal of a cross edge marker.
const CROSS_ARM: f32 = 4.0;
/// Largest radius a routed corner is rounded by.
const CORNER_RADIUS: f32 = 8.0;

/// The palette's colours, each as a signal of its own so a command binds only
/// the colour it paints with.
struct Colors {
    node_fill: Computed<WorkingColor>,
    node_border: Computed<WorkingColor>,
    edge: Computed<WorkingColor>,
    cluster: Computed<WorkingColor>,
    border: Computed<WorkingColor>,
}

impl Colors {
    fn new(palette: &Computed<Palette>) -> Self {
        Self {
            node_fill: palette.map(|palette| palette.node_fill).computed(),
            node_border: palette.map(|palette| palette.node_border).computed(),
            edge: palette.map(|palette| palette.edge).computed(),
            cluster: palette.map(|palette| palette.cluster).computed(),
            border: palette.map(|palette| palette.border).computed(),
        }
    }
}

/// The ordinary stroke of an outline, a frame or a marker.
const fn outline_stroke() -> StrokeStyle {
    StrokeStyle::new(STROKE)
}

/// The dashed stroke of a divider, a lifeline or a dotted edge.
fn dashed_stroke(width: f32) -> StrokeStyle {
    StrokeStyle::new(width).dash(DASH, 0.0)
}

/// Paints one diagram at its natural size, with its top-left at `origin`.
pub fn diagram(cx: &mut Cx, layout: &DiagramLayout, palette: &Computed<Palette>, origin: Point) {
    let colors = Colors::new(palette);
    let at = |point: Point| Point::new(point.x + origin.x, point.y + origin.y);

    for cluster in &layout.clusters {
        let frame = offset_rect(cluster.frame, origin);
        cx.fill(frame, colors.cluster.clone());
        cx.stroke(frame, outline_stroke(), colors.border.clone());
    }

    for fragment in &layout.fragments {
        let frame = offset_rect(fragment.frame, origin);
        cx.stroke(frame, outline_stroke(), colors.border.clone());
        for divider in &fragment.dividers {
            let y = divider + origin.y;
            cx.stroke(
                Line::new(Point::new(frame.min_x(), y), Point::new(frame.max_x(), y)),
                dashed_stroke(STROKE),
                colors.border.clone(),
            );
        }
    }

    for lifeline in &layout.lifelines {
        cx.stroke(
            Line::new(
                at(Point::new(lifeline.x, lifeline.top)),
                at(Point::new(lifeline.x, lifeline.bottom)),
            ),
            dashed_stroke(STROKE),
            colors.border.clone(),
        );
    }

    for edge in &layout.edges {
        connector(cx, edge, &colors.edge, origin);
    }

    for node in &layout.nodes {
        let outline = shape::outline(node.shape, offset_rect(node.frame, origin));
        if let Some(body) = outline.body {
            cx.fill(body.clone(), colors.node_fill.clone());
            cx.stroke(body, outline_stroke(), colors.node_border.clone());
        }
        for detail in outline.details {
            cx.stroke(detail, outline_stroke(), colors.node_border.clone());
        }
    }
}

fn offset_rect(rect: Rect, origin: Point) -> Rect {
    Rect::new(
        Point::new(rect.x() + origin.x, rect.y() + origin.y),
        *rect.size(),
    )
}

/// Paints one edge: its routed polyline, then a decoration at each end.
fn connector(cx: &mut Cx, edge: &Edge, color: &Computed<WorkingColor>, origin: Point) {
    let points: Vec<Point> = edge
        .points
        .iter()
        .map(|point| Point::new(point.x + origin.x, point.y + origin.y))
        .collect();
    let Some(path) = edge_path(&points) else {
        return;
    };

    let width = match edge.stroke {
        EdgeStroke::Thick => THICK_STROKE,
        EdgeStroke::Normal | EdgeStroke::Dotted => STROKE,
    };
    let style = if edge.stroke == EdgeStroke::Dotted {
        dashed_stroke(width)
    } else {
        StrokeStyle::new(width)
    };
    cx.stroke(path, style, color.clone());

    marker(cx, edge.start_marker, points[0], points[1], color);
    let last = points.len() - 1;
    marker(cx, edge.end_marker, points[last], points[last - 1], color);
}

/// The stroked path of one edge's routed polyline.
///
/// `None` when there is nothing to stroke: a polyline needs two points before
/// it is a line.
fn edge_path(points: &[Point]) -> Option<Path> {
    let [first, _, ..] = points else {
        return None;
    };
    Some(route(Path::builder().move_to(*first), points).build())
}

/// Appends the routed polyline to `path`, rounding each interior corner.
///
/// `points` starts with the point `path` was already moved to, and holds at
/// least one more.
fn route(mut path: PathBuilder, points: &[Point]) -> PathBuilder {
    let [first, interior @ .., last] = points else {
        unreachable!("a routed polyline has two ends")
    };

    let mut previous = *first;
    for (index, corner) in interior.iter().enumerate() {
        let next = interior.get(index + 1).copied().unwrap_or(*last);
        path = fillet(path, previous, *corner, next);
        previous = *corner;
    }
    path.line_to(*last)
}

/// Rounds the corner at `corner`, where the segment from `from` meets the one
/// to `to`, with the largest arc of at most [`CORNER_RADIUS`] that does not eat
/// into more than half of either segment.
///
/// The canvas draws arcs around a centre, not tangent to two lines, so the
/// tangent arc is worked out here: its two ends sit the same distance from the
/// corner along each segment, and its centre sits on the bisector of the angle
/// between them. The straight run up to the arc's start is the canvas's own
/// join from the current point.
fn fillet(path: PathBuilder, from: Point, corner: Point, to: Point) -> PathBuilder {
    let (incoming, outgoing) = (offset(corner, from), offset(corner, to));
    let (incoming_length, outgoing_length) =
        (incoming.0.hypot(incoming.1), outgoing.0.hypot(outgoing.1));
    if incoming_length <= f32::EPSILON || outgoing_length <= f32::EPSILON {
        return path.line_to(corner);
    }
    let (first, second) = (
        (incoming.0 / incoming_length, incoming.1 / incoming_length),
        (outgoing.0 / outgoing_length, outgoing.1 / outgoing_length),
    );
    // The angle between the segments, as seen from the corner: a right-angled
    // turn is a quarter-turn, a straight run through is a half-turn.
    let angle = (first.0.mul_add(second.0, first.1 * second.1))
        .clamp(-1.0, 1.0)
        .acos();
    let half = angle / 2.0;
    if half <= f32::EPSILON || core::f32::consts::FRAC_PI_2 - half <= f32::EPSILON {
        // A reversal or a straight run: there is no corner to round.
        return path.line_to(corner);
    }

    // How far from the corner the arc touches each segment, and the radius
    // that puts it there.
    let reach = (CORNER_RADIUS / half.tan())
        .min(incoming_length / 2.0)
        .min(outgoing_length / 2.0);
    let radius = reach * half.tan();
    let start = Point::new(
        first.0.mul_add(reach, corner.x),
        first.1.mul_add(reach, corner.y),
    );
    let end = Point::new(
        second.0.mul_add(reach, corner.x),
        second.1.mul_add(reach, corner.y),
    );
    let (bisector_x, bisector_y) = (first.0 + second.0, first.1 + second.1);
    let bisector_length = bisector_x.hypot(bisector_y);
    let distance = radius / half.sin();
    let centre = Point::new(
        (bisector_x / bisector_length).mul_add(distance, corner.x),
        (bisector_y / bisector_length).mul_add(distance, corner.y),
    );

    let begin = (start.y - centre.y).atan2(start.x - centre.x);
    let finish = (end.y - centre.y).atan2(end.x - centre.x);
    // The arc turns the short way round, which is the way the edge does.
    let mut sweep = finish - begin;
    if sweep > core::f32::consts::PI {
        sweep -= core::f32::consts::TAU;
    } else if sweep < -core::f32::consts::PI {
        sweep += core::f32::consts::TAU;
    }
    path.arc(centre, (radius, radius), begin, sweep)
}

/// The vector from `from` to `to`.
fn offset(from: Point, to: Point) -> (f32, f32) {
    (to.x - from.x, to.y - from.y)
}

/// Paints the decoration at one end of an edge.
///
/// `tip` is the end point; `from` is the neighbouring point, which is what
/// gives the decoration its direction.
fn marker(
    cx: &mut Cx,
    marker: EdgeMarker,
    tip: Point,
    from: Point,
    color: &Computed<WorkingColor>,
) {
    if marker == EdgeMarker::None {
        return;
    }
    let (dx, dy) = (tip.x - from.x, tip.y - from.y);
    let length = dx.hypot(dy);
    if length <= f32::EPSILON {
        return;
    }
    let (ux, uy) = (dx / length, dy / length);

    match marker {
        EdgeMarker::Arrow => {
            let back = Point::new(
                (-ux).mul_add(ARROW_LENGTH, tip.x),
                (-uy).mul_add(ARROW_LENGTH, tip.y),
            );
            let head = Path::builder()
                .move_to(tip)
                .line_to(Point::new(
                    (-uy).mul_add(ARROW_HALF_WIDTH, back.x),
                    ux.mul_add(ARROW_HALF_WIDTH, back.y),
                ))
                .line_to(Point::new(
                    uy.mul_add(ARROW_HALF_WIDTH, back.x),
                    (-ux).mul_add(ARROW_HALF_WIDTH, back.y),
                ))
                .close()
                .build();
            cx.fill(head, color.clone());
        }
        EdgeMarker::Circle => {
            cx.stroke(
                Circle::new(tip, MARKER_RADIUS),
                outline_stroke(),
                color.clone(),
            );
        }
        EdgeMarker::Cross => {
            for (start, end) in [
                (
                    Point::new(tip.x - CROSS_ARM, tip.y - CROSS_ARM),
                    Point::new(tip.x + CROSS_ARM, tip.y + CROSS_ARM),
                ),
                (
                    Point::new(tip.x - CROSS_ARM, tip.y + CROSS_ARM),
                    Point::new(tip.x + CROSS_ARM, tip.y - CROSS_ARM),
                ),
            ] {
                cx.stroke(Line::new(start, end), outline_stroke(), color.clone());
            }
        }
        EdgeMarker::None => {}
    }
}

#[cfg(test)]
mod tests {
    use waterui_core::layout::{Point, Rect, Size};

    use waterui_text::FontCollection;

    use super::{CORNER_RADIUS, edge_path};
    use crate::engine::render;
    use crate::layout::DiagramLayout;
    use crate::shape;

    /// A flowchart whose edges turn: `TD` stacks the nodes, and `D --> A` runs
    /// back up past them, so every edge is a routed polyline with corners in it
    /// rather than one straight segment.
    const TURNING: &str = "\
flowchart TD
    A[Start] --> B{Ready?}
    B -->|yes| C([Go])
    B -->|no| D[(Wait)]
    D --> A
";

    /// How far a piece of geometry may sit outside where it belongs before the
    /// diagram is wrong rather than rounded.
    const SLACK: f32 = 0.5;

    fn diagram() -> DiagramLayout {
        render(TURNING, FontCollection::system()).expect("the flowchart lays out")
    }

    /// The box every point of `points` lies in.
    fn hull(points: &[Point]) -> Rect {
        let (mut min, mut max) = (points[0], points[0]);
        for point in points {
            min = Point::new(min.x.min(point.x), min.y.min(point.y));
            max = Point::new(max.x.max(point.x), max.y.max(point.y));
        }
        Rect::new(min, Size::new(max.x - min.x, max.y - min.y))
    }

    /// How far `inner` sticks out of `outer`, in the worst direction.
    fn escape(inner: Rect, outer: Rect) -> f32 {
        (outer.x() - inner.x())
            .max(inner.max_x() - outer.max_x())
            .max(outer.y() - inner.y())
            .max(inner.max_y() - outer.max_y())
            .max(0.0)
    }

    fn canvas(diagram: &DiagramLayout) -> Rect {
        Rect::new(Point::zero(), diagram.size)
    }

    /// Nothing this crate paints for a node may leave the diagram's own bounds:
    /// the canvas is exactly `diagram.size` and anything outside it is clipped
    /// away by the container.
    #[core::prelude::v1::test]
    fn every_node_outline_stays_on_the_canvas() {
        let diagram = diagram();
        let canvas = canvas(&diagram);
        for node in &diagram.nodes {
            let outline = shape::outline(node.shape, node.frame);
            for path in outline.body.iter().chain(&outline.details) {
                let bounds = path.bounds();
                assert!(
                    escape(bounds, canvas) <= SLACK,
                    "node `{}` draws {bounds:?}, outside the {canvas:?} canvas",
                    node.id
                );
            }
        }
    }

    /// The routed path of an edge is its polyline with the corners rounded, so
    /// it can never travel further than the polyline itself does — a fillet
    /// lives inside the corner it rounds.
    ///
    /// This is what a broken corner-rounding computation breaks: the tangent
    /// distance comes out inverted, the arc's centre lands on the wrong side,
    /// and the path shoots off along the last segment's direction far past the
    /// node it was heading for. `D --> A` here left the diagram entirely.
    #[core::prelude::v1::test]
    fn every_edge_path_follows_its_own_polyline() {
        let diagram = diagram();
        let canvas = canvas(&diagram);
        for edge in &diagram.edges {
            let path = edge_path(&edge.points).expect("a routed edge draws something");
            let bounds = path.bounds();
            let corridor = hull(&edge.points);
            assert!(
                escape(bounds, corridor) <= CORNER_RADIUS,
                "edge `{}` draws {bounds:?}, off the {corridor:?} its points span",
                edge.id
            );
            assert!(
                escape(bounds, canvas) <= SLACK,
                "edge `{}` draws {bounds:?}, outside the {canvas:?} canvas",
                edge.id
            );
        }
    }

    /// An edge starts and ends on a node. Mermaid trims the polyline to the
    /// node's border, so each end lands on the box of the node it belongs to —
    /// a diamond's slanted face is inside its box, which is why this is stated
    /// as the box rather than its outline.
    #[core::prelude::v1::test]
    fn every_edge_ends_on_a_node() {
        let diagram = diagram();
        for edge in &diagram.edges {
            for (which, end) in [("start", edge.points.first()), ("end", edge.points.last())] {
                let end = *end.expect("a routed edge has endpoints");
                let touched = diagram
                    .nodes
                    .iter()
                    .find(|node| escape(Rect::new(end, Size::new(0.0, 0.0)), node.frame) <= SLACK);
                assert!(
                    touched.is_some(),
                    "the {which} of edge `{}` is at {end:?}, on no node",
                    edge.id
                );
            }
        }
    }
}
