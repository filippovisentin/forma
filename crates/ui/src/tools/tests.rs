//! Tests of the interactive tools.

use super::*;

fn run(tool: &mut Tool, pts: &[Point3]) -> Vec<String> {
    for p in pts {
        match tool.feed_point(*p) {
            Step::Done(c) | Step::Emit(c) => return c,
            Step::Cancel(e) => panic!("{e}"),
            Step::Continue => {}
        }
    }
    panic!("tool did not finish");
}

#[test]
fn line_and_box_emit_commands() {
    let mut t = Tool::new(ToolKind::Line, Plane::TOP, false, Point3::ORIGIN, vec![]);
    let c = run(&mut t, &[Point3::ORIGIN, Point3::new(100.0, 0.0, 0.0)]);
    assert_eq!(c, vec!["Line 0,0,0 100,0,0"]);
    let mut b = Tool::new(ToolKind::Box, Plane::TOP, false, Point3::ORIGIN, vec![]);
    b.feed_point(Point3::ORIGIN);
    b.feed_point(Point3::new(600.0, 400.0, 0.0));
    let Step::Done(c) = b.feed_number(18.0) else {
        panic!()
    };
    assert_eq!(c, vec!["Box 0,0,0 600,400,0 18 0,0,1"]);
}

#[test]
fn transform_tools_need_selection() {
    let mut t = Tool::new(ToolKind::Move, Plane::TOP, false, Point3::ORIGIN, vec![]);
    assert_eq!(t.want(), Want::Selection);
    assert!(matches!(t.enter(false), Step::Cancel(_)));
    let mut t = Tool::new(ToolKind::Move, Plane::TOP, false, Point3::ORIGIN, vec![]);
    assert!(matches!(t.enter(true), Step::Continue));
    let c = run(&mut t, &[Point3::ORIGIN, Point3::new(0.0, 50.0, 0.0)]);
    assert_eq!(c, vec!["Move 0,0,0 0,50,0"]);
}

#[test]
fn polyline_closes_with_c() {
    let mut t = Tool::new(
        ToolKind::Polyline,
        Plane::TOP,
        false,
        Point3::ORIGIN,
        vec![],
    );
    for p in [
        Point3::ORIGIN,
        Point3::new(1.0, 0.0, 0.0),
        Point3::new(1.0, 1.0, 0.0),
    ] {
        t.feed_point(p);
    }
    let Some(Step::Done(c)) = t.option("C") else {
        panic!()
    };
    assert_eq!(c, vec!["Polyline 0,0,0 1,0,0 1,1,0 c"]);
}

#[test]
fn rotate_by_reference_points() {
    let mut t = Tool::new(ToolKind::Rotate, Plane::TOP, true, Point3::ORIGIN, vec![]);
    t.feed_point(Point3::ORIGIN);
    t.feed_point(Point3::new(1.0, 0.0, 0.0));
    let Step::Done(c) = t.feed_point(Point3::new(0.0, 2.0, 0.0)) else {
        panic!()
    };
    assert_eq!(c, vec!["Rotate 0,0,0 90 0,0,1"]);
}

#[test]
fn typed_points_in_cplane() {
    let front = Plane::FRONT;
    let p = parse_typed_point("100,50", &front, None, None).unwrap();
    assert_eq!(p, Point3::new(100.0, 0.0, 50.0));
    let base = Some(Point3::new(10.0, 0.0, 0.0));
    let q = parse_typed_point("@0,20", &front, base, None).unwrap();
    assert_eq!(q, Point3::new(10.0, 0.0, 20.0));
    let r = parse_typed_point("30", &Plane::TOP, base, Some(Point3::new(10.0, 5.0, 0.0))).unwrap();
    assert!(r.distance_to(Point3::new(10.0, 30.0, 0.0)) < 1e-9);
    let s = parse_typed_point("@10<90", &Plane::TOP, base, None).unwrap();
    assert!(s.distance_to(Point3::new(10.0, 10.0, 0.0)) < 1e-9);
}

#[test]
fn new_curve_tools_emit_commands() {
    let mut p = Tool::new(ToolKind::Polygon, Plane::TOP, false, Point3::ORIGIN, vec![]);
    assert!(matches!(p.feed_number(6.0), Step::Continue));
    p.feed_point(Point3::ORIGIN);
    let Step::Done(c) = p.feed_point(Point3::new(10.0, 0.0, 0.0)) else {
        panic!()
    };
    assert_eq!(c, vec!["Polygon 0,0,0 10,0,0 6 0,0,1"]);
    let mut e = Tool::new(ToolKind::Ellipse, Plane::TOP, false, Point3::ORIGIN, vec![]);
    e.feed_point(Point3::ORIGIN);
    e.feed_point(Point3::new(30.0, 0.0, 0.0));
    let Step::Done(c) = e.feed_point(Point3::new(5.0, 10.0, 0.0)) else {
        panic!()
    };
    assert_eq!(c, vec!["Ellipse 0,0,0 30,0,0 10 0,0,1"]);
    let mut k = Tool::new(ToolKind::Curve, Plane::TOP, false, Point3::ORIGIN, vec![]);
    for x in [0.0, 1.0, 2.0] {
        k.feed_point(Point3::new(x, x, 0.0));
    }
    let Step::Done(c) = k.enter(false) else {
        panic!()
    };
    assert_eq!(c, vec!["Curve 0,0,0 1,1,0 2,2,0"]);
}

#[test]
fn scale1d_by_reference_and_selection_commands() {
    let mut t = Tool::new(ToolKind::Scale1D, Plane::TOP, true, Point3::ORIGIN, vec![]);
    t.feed_point(Point3::ORIGIN);
    t.feed_point(Point3::new(10.0, 0.0, 0.0));
    let Step::Done(c) = t.feed_point(Point3::new(25.0, 3.0, 0.0)) else {
        panic!()
    };
    assert_eq!(c, vec!["Scale1D 0,0,0 2.5 10,0,0"]);
    assert_eq!(
        ToolKind::selection_command("hide"),
        Some(ToolKind::OnSel("Hide"))
    );
    assert_eq!(
        ToolKind::selection_command("dir"),
        Some(ToolKind::OnSel("Flip"))
    );
    let mut h = Tool::new(
        ToolKind::OnSel("Hide"),
        Plane::TOP,
        false,
        Point3::ORIGIN,
        vec![],
    );
    assert_eq!(h.want(), Want::Selection);
    let Step::Done(c) = h.enter(true) else {
        panic!()
    };
    assert_eq!(c, vec!["Hide"]);
    let f = Tool::new(
        ToolKind::OnSel("ProjectToCPlane"),
        Plane::FRONT,
        true,
        Point3::ORIGIN,
        vec![],
    );
    assert_eq!(f.instant_line(), "ProjectToCPlane 0,-1,0 0,0,0");
}

#[test]
fn prompt_options_follow_the_step() {
    let mut t = Tool::new(
        ToolKind::Polyline,
        Plane::TOP,
        false,
        Point3::ORIGIN,
        vec![],
    );
    assert!(t.options().is_empty());
    t.feed_point(Point3::ORIGIN);
    let labels: Vec<String> = t.options().into_iter().map(|o| o.label).collect();
    assert_eq!(labels, vec!["Undo"]);
    t.feed_point(Point3::new(10.0, 0.0, 0.0));
    t.feed_point(Point3::new(10.0, 10.0, 0.0));
    let opts = t.options();
    assert_eq!(opts[0].action, OptAction::Word("Close"));
    assert_eq!(opts[1].action, OptAction::Word("Undo"));

    let mut o = Tool::new(ToolKind::Offset, Plane::TOP, true, Point3::ORIGIN, vec![]);
    o.distance = 10.0;
    assert_eq!(o.options()[0].label, "Distance=10");
    assert_eq!(o.option_value("distance"), Some(10.0));
    o.set_option("Distance", 2.5).expect("valid");
    assert_eq!(o.options()[0].label, "Distance=2.5");
    assert!(o.set_option("Distance", -1.0).is_err());

    let mut p = Tool::new(ToolKind::Polygon, Plane::TOP, false, Point3::ORIGIN, vec![]);
    assert_eq!(p.options()[0].label, "NumSides=5");
    p.set_option("NumSides", 8.0).expect("valid");
    assert_eq!(p.option_value("NumSides"), Some(8.0));
    assert!(p.set_option("NumSides", 2.0).is_err());
    let f = Tool::new(ToolKind::Fillet, Plane::TOP, false, Point3::ORIGIN, vec![]);
    assert_eq!(f.options()[0].action, OptAction::Value("Radius"));
}

#[test]
fn aliases_round_trip() {
    let kinds = ToolKind::CURVES
        .iter()
        .chain(&ToolKind::CURVE_TOOLS)
        .chain(&ToolKind::SURFACES)
        .chain(&ToolKind::TRANSFORMS)
        .chain(&ToolKind::ANALYZE)
        .chain(&ToolKind::EDIT);
    for k in kinds {
        if let (Some(a), ToolKind::OnSel(_)) = (k.alias(), k) {
            assert_eq!(ToolKind::selection_command(a), Some(*k), "{a}");
        } else if let Some(a) = k.alias() {
            assert_eq!(ToolKind::from_name(a), Some(*k), "{a}");
        }
        assert!(!k.description().is_empty(), "{}", k.name());
    }
}
