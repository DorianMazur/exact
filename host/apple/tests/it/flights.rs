//! Shared-element flights on Apple (LLP 1013.000 D4): a handoff sends
//! `flight` before the leaver's destroy, then the curve's progress as
//! `present … "flight"`, then `land` once it settles.

use exact_apple::Host;
use exact_kernel::MonospaceMeasurer;
use exact_runner::{DataError, DataSource, Event, Value};

struct NoData;
impl DataSource for NoData {
    fn query(&mut self, s: &str, _: &[Value]) -> Result<Value, DataError> {
        Err(DataError::UnknownSource(s.into()))
    }
}

fn view<D: DataSource>(host: &Host<D>, test_id: &str) -> Option<u32> {
    let k = host.runner().kernel();
    let key = *k.find_by_test_id(test_id).first()?;
    Some(k.node_by_key(key).unwrap().id)
}

fn progress(batch: &str, id: u32) -> Option<f64> {
    let marker = format!("\"op\":\"present\",\"id\":{id},\"property\":\"flight\",\"x\":");
    let at = batch.rfind(&marker)?;
    batch[at + marker.len()..]
        .split([',', '}'])
        .next()?
        .parse()
        .ok()
}

fn boot() -> Host<NoData> {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/shared-elements.contract"
    ))
    .unwrap();
    let plan = contract::compile(&src).unwrap();
    let (host, first) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    assert!(!first.contains("\"op\":\"flight\""), "{first}");
    host
}

#[test]
fn a_handoff_flies_on_the_leavers_curve_and_lands() {
    let mut host = boot();
    let (thumb, toggle) = (
        view(&host, "thumb").unwrap(),
        view(&host, "toggle").unwrap(),
    );
    let open = host.dispatch_at(toggle, Event::Press, 100.0);
    let large = view(&host, "large").unwrap();
    let flight = format!("{{\"op\":\"flight\",\"id\":{large},\"from\":{thumb}}}");
    let destroy = format!("{{\"op\":\"destroy\",\"id\":{thumb}}}");
    let (at, gone) = (open.find(&flight), open.find(&destroy));
    assert!(at.is_some(), "{open}");
    assert!(
        gone.is_none_or(|g| at.unwrap() < g),
        "captured before the destroy: {open}"
    );
    assert!(open.contains("\"motion\":true"), "{open}");
    let mid = host.tick(250.0);
    let p = progress(&mid, large).expect("progress mid-flight");
    assert!(p > 0.3 && p < 0.9, "ease at half time: {p}");
    let done = host.tick(401.0);
    assert!(
        done.contains(&format!("{{\"op\":\"land\",\"id\":{large}}}")),
        "{done}"
    );
    let after = host.tick(500.0);
    assert!(!after.contains("\"op\":\"land\""), "lands once: {after}");
}

#[test]
fn closing_mid_flight_ends_the_first_flight_and_starts_the_reverse() {
    let mut host = boot();
    let toggle = view(&host, "toggle").unwrap();
    host.dispatch_at(toggle, Event::Press, 100.0);
    let large = view(&host, "large").unwrap();
    host.tick(200.0);
    let back = host.dispatch_at(toggle, Event::Press, 220.0);
    let thumb = view(&host, "thumb").unwrap();
    assert!(
        back.contains(&format!(
            "{{\"op\":\"flight\",\"id\":{thumb},\"from\":{large}}}"
        )),
        "{back}"
    );
    assert!(
        !back.contains(&format!("{{\"op\":\"land\",\"id\":{large}}}")),
        "a destroyed arriver doesn't land: {back}"
    );
    let done = host.tick(521.0);
    assert!(
        done.contains(&format!("{{\"op\":\"land\",\"id\":{thumb}}}")),
        "{done}"
    );
    // The interrupted flight's curve went with its view: nothing keeps
    // the clock running.
    let rest = host.tick(600.0);
    assert!(rest.contains("\"motion\":false"), "{rest}");
}

/// A spring flight lands where UIKit's spring animators finish (within
/// 1/1000 of its travel, moving under 1/20 of it a second): 0.37 s for
/// Signal's photo zoom spring (critically damped, response 0.25), which the
/// engine's own rest would have run to 0.78 s.
#[test]
fn a_spring_flight_lands_as_uikits_spring_finishes() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/shared-elements.contract"
    ))
    .unwrap()
    .replace(
        "-exact-layout-transition=\"300ms ease\"",
        "-exact-layout-transition=\"-exact-spring(631.655, 50.265, 1)\"",
    );
    assert!(src.contains("-exact-spring(631.655"));
    let plan = contract::compile(&src).unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let toggle = view(&host, "toggle").unwrap();
    host.dispatch_at(toggle, Event::Press, 100.0);
    let large = view(&host, "large").unwrap();
    let land = format!("{{\"op\":\"land\",\"id\":{large}}}");
    let early = host.tick(440.0);
    assert!(!early.contains(&land), "still flying at 0.34 s: {early}");
    let p = progress(&early, large).expect("progress at 0.34 s");
    assert!(p > 0.99 && p < 0.9995, "nearly there: {p}");
    let done = host.tick(520.0);
    assert!(done.contains(&land), "landed by 0.42 s: {done}");
}

/// A slow, lightly damped spring crosses its target slowly long before it
/// settles: a flight on it lands only once it stays within the bounds, and
/// a seek past the first such crossing does not land it early.
#[test]
fn a_flight_on_a_bouncy_spring_lands_only_once_it_stays_settled() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contract/corpus/shared-elements.contract"
    ))
    .unwrap()
    .replace(
        "-exact-layout-transition=\"300ms ease\"",
        "-exact-layout-transition=\"-exact-spring(1, 1, 1)\"",
    );
    let plan = contract::compile(&src).unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let toggle = view(&host, "toggle").unwrap();
    host.dispatch_at(toggle, Event::Press, 100.0);
    let large = view(&host, "large").unwrap();
    let land = format!("{{\"op\":\"land\",\"id\":{large}}}");
    // Past the slow crossing near 6 s, the next excursion is 2% of the travel.
    let seek = host.tick(6600.0);
    assert!(!seek.contains(&land), "not landed mid-oscillation: {seek}");
}

#[test]
fn with_the_first_frame_rule_a_flight_lands_from_the_frame_that_started_it() {
    let mut host = boot();
    host.start_on_frame(true, 0.0);
    let toggle = view(&host, "toggle").unwrap();
    host.dispatch_at(toggle, Event::Press, 100.0);
    let large = view(&host, "large").unwrap();
    let first = host.tick_at(110.0, 116.0);
    assert_eq!(
        progress(&first, large),
        Some(0.0),
        "the first frame shows the start: {first}"
    );
    let land = format!("{{\"op\":\"land\",\"id\":{large}}}");
    assert!(
        !host.tick_at(400.0, 405.0).contains(&land),
        "300 ms from the commit is not its end"
    );
    let done = host.tick_at(420.0, 417.0);
    assert!(done.contains(&land), "300 ms from the frame is: {done}");
}

#[test]
fn shared_content_keeps_its_player_and_control_until_the_owner_leaves() {
    let plan = contract::compile(include_str!(
        "../../../../contract/corpus/shared-content.contract"
    ))
    .unwrap();
    let (mut host, _) = Host::boot(
        &plan.encode(),
        NoData,
        Box::new(MonospaceMeasurer::default()),
        390.0,
        844.0,
    )
    .unwrap();
    let player = view(&host, "video").unwrap();
    let control = view(&host, "count").unwrap();
    let assert_count = |host: &Host<NoData>, expected: &str| {
        let k = host.runner().kernel();
        let button = k.node(view(host, "count").unwrap()).unwrap();
        let label = k.node(button.children()[0]).unwrap();
        assert_eq!(label.props.str(exact_kernel::PropId::Text), Some(expected));
    };
    let assert_place = |host: &Host<NoData>, name: &str| {
        let slot = view(host, name).unwrap();
        let kernel = host.runner().kernel();
        assert_eq!(kernel.node(slot).unwrap().children(), [player, control]);
        assert!(kernel
            .node(view(host, "owner").unwrap())
            .unwrap()
            .children()
            .is_empty());
        assert_eq!(kernel.find_by_test_id("video").len(), 1);
    };
    assert_place(&host, "card");
    for (at, name) in [(100.0, "detail"), (200.0, "card"), (500.0, "detail")] {
        let move_batch = host.dispatch_at(view(&host, "toggle").unwrap(), Event::Press, at);
        let slot = view(&host, name).unwrap();
        assert!(move_batch.contains(&format!(r#""op":"flight","id":{slot},"from":"#)));
        assert_place(&host, name);
        host.tick(at + 50.0);
    }
    host.tick(1100.0);
    host.dispatch_at(control, Event::Press, 1110.0);
    assert_count(&host, "Count 1");
    let back = host.dispatch_at(view(&host, "toggle").unwrap(), Event::Press, 1120.0);
    assert!(!back.contains(&format!(r#""op":"destroy","id":{player}}}"#)));
    assert_place(&host, "card");
    host.dispatch_at(control, Event::Press, 1620.0);
    assert_count(&host, "Count 2");
    host.dispatch_at(view(&host, "remove").unwrap(), Event::Press, 1700.0);
    assert!(host.runner().kernel().node(player).is_none());
    assert!(host.runner().kernel().node(control).is_none());
    host.dispatch_at(view(&host, "remove").unwrap(), Event::Press, 1800.0);
    assert_ne!(view(&host, "video"), Some(player));
    assert_count(&host, "Count 0");
}
