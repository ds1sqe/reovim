use reovim_client_tui::context::{self, Error};
use reovim_kernel_interface::{Contract, Message, ResourceId};
use reovim_module_text::api::{self, Observation, Unit, Window};
use unicode_segmentation::UnicodeSegmentation;

fn encoded(units: &[Unit]) -> String {
    units
        .iter()
        .map(|unit| match unit {
            Unit::Scalar(ch) => *ch,
            Unit::Opaque(_) => '\0',
        })
        .collect()
}

fn observation(units: &[Unit]) -> Observation {
    let bytes = encoded(units).len() as u64;
    api::observation(
        &Message {
            contract: Contract::new("text.observe", 1),
            data: [
                4_u64.to_le_bytes(),
                0_u64.to_le_bytes(),
                bytes.to_le_bytes(),
            ]
            .concat()
            .into(),
            resources: [2, 2, 1, 4, 3, 5].map(ResourceId).into(),
        },
        ResourceId(2),
    )
    .unwrap()
}

fn chunk(
    observed: &Observation,
    units: &[Unit],
    start: u64,
    count: u64,
) -> Window {
    let start = usize::try_from(start).unwrap();
    let end = (start + usize::try_from(count).unwrap()).min(units.len());
    Window {
        observed: observed.observed(),
        snapshot: observed.snapshot(),
        range: start as u64..end as u64,
        units: units[start..end].to_vec(),
        eof: end == units.len(),
    }
}

fn compare(units: &[Unit], wanted: usize) {
    let observed = observation(units);
    let text = encoded(units);
    let mut boundaries = vec![0];
    for cluster in text.graphemes(true) {
        boundaries.push(boundaries.last().unwrap() + cluster.chars().count());
    }
    let start = *boundaries.iter().rev().find(|p| **p <= wanted).unwrap();
    let end = *boundaries
        .iter()
        .rfind(|p| **p >= start && **p - start <= 1024)
        .unwrap();
    let result = context::read(&observed, wanted as u64, |start, count| {
        assert!(count <= 1024, "named Text read bound");
        Ok::<_, ()>(chunk(&observed, units, start, count))
    });
    if start == end && end != units.len() {
        assert!(matches!(result, Err(Error::GraphemeTooLarge)));
        return;
    }
    let result = result.unwrap();
    assert_eq!(result.range, start as u64..end as u64);
    assert_eq!(result.units, units[start..end]);
    assert_eq!(result.eof, end == units.len());
}

#[test]
fn chunked_context_matches_whole_text_for_crossing_sequences() {
    let cases = [
        format!("{}\r\nend", "a".repeat(1023)),
        format!("{}👩{}\u{200d}💻 end", "a".repeat(1023), "\u{301}".repeat(20)),
        format!("{}end", "🇦".repeat(5000)),
        format!("{}क end", "क्".repeat(800)),
    ];
    for text in cases {
        let units: Vec<_> = text.chars().map(Unit::Scalar).collect();
        for wanted in [0, 1023, 1024, units.len() - 1, units.len()] {
            compare(&units, wanted);
        }
    }
}

#[test]
fn opaque_units_break_context_without_becoming_replacement_text() {
    let mut units = vec![Unit::Scalar('🇦'); 1023];
    units.extend([
        Unit::Opaque(255),
        Unit::Scalar('\u{301}'),
        Unit::Scalar('x'),
    ]);
    compare(&units, 1023);
    compare(&units, 1024);
    compare(&[], 0);
}

#[test]
fn foreign_snapshots_and_missing_data_do_not_become_ready_windows() {
    let units = [Unit::Scalar('a')];
    let observed = observation(&units);
    let mut partial = chunk(&observed, &units, 0, 1);
    partial.eof = false;
    assert!(context::Window::complete(partial).is_err());
    let result = context::read(&observed, 0, |start, count| {
        let mut value = chunk(&observed, &units, start, count);
        value.snapshot = ResourceId(99);
        Ok::<_, ()>(value)
    });
    assert!(matches!(result, Err(Error::InvalidChunk)));
    assert!(matches!(
        context::read(&observed, 0, |_, _| Err::<Window, _>("lost")),
        Err(Error::Read("lost"))
    ));
}

#[test]
fn oversized_unselected_cluster_does_not_force_unbounded_lookahead() {
    let text = format!("{}{} tail", "a".repeat(1023), "\u{301}".repeat(10000));
    let units: Vec<_> = text.chars().map(Unit::Scalar).collect();
    let observed = observation(&units);
    let result = context::read(&observed, 0, |start, count| {
        assert!(start + count <= 2048, "two bounded lookahead chunks");
        Ok::<_, ()>(chunk(&observed, &units, start, count))
    })
    .unwrap();
    assert_eq!(result.range, 0..1022);
    assert_eq!(result.units, units[..1022]);
    // A large earlier cluster does not cap the document or hide later text.
    compare(&units, units.len() - 2);
}

#[test]
fn nonzero_windows_keep_display_width_based_tab_stops() {
    for (prefix, phase) in [("漢", 2), ("a\u{301}", 1), ("👩\u{200d}💻", 2)]
    {
        let text = format!("{}{prefix}\tX", "a".repeat(1024));
        let units: Vec<_> = text.chars().map(Unit::Scalar).collect();
        let observed = observation(&units);
        let wanted = 1024 + prefix.chars().count() as u64;
        let value = context::read(&observed, wanted, |start, count| {
            Ok::<_, ()>(chunk(&observed, &units, start, count))
        })
        .unwrap();
        assert_eq!(value.column(), Some(phase));
        let frame =
            reovim_client_tui::viewport::project(&value, 1, 8, 2, 0).unwrap();
        assert_eq!(frame.lines, [format!("{}X", " ".repeat(4 - phase))]);
        assert_eq!(frame.cursor, Some((u16::try_from(4 - phase).unwrap(), 0)));
    }
}

#[test]
fn unknown_oversized_prefix_is_not_guessed_and_real_stops_restore_context() {
    let text = format!("a{}\tX\n\tY", "\u{301}".repeat(2048));
    let units: Vec<_> = text.chars().map(Unit::Scalar).collect();
    let observed = observation(&units);
    let read =
        |start, count| Ok::<_, ()>(chunk(&observed, &units, start, count));
    let unknown = context::read(&observed, 2049, read).unwrap();
    assert_eq!(unknown.column(), None);
    assert!(matches!(
        reovim_client_tui::viewport::project(&unknown, 0, 8, 3, 0),
        Err(reovim_client_tui::viewport::Error::ColumnContext)
    ));
    let known = context::read(&observed, 2050, read).unwrap();
    assert_eq!(known.column(), Some(0));
    let frame =
        reovim_client_tui::viewport::project(&known, 0, 8, 3, 0).unwrap();
    assert_eq!(frame.lines, ["X", "    Y"]);
}
