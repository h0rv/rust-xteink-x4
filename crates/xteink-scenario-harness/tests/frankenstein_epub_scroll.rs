use std::{collections::BTreeSet, env, path::PathBuf};

use xteink_scenario_harness::ScenarioHarness;
use xteink_ui::{App, AppScreen, Button, MockFileSystem};

fn setup_harness() -> ScenarioHarness {
    let mut app = App::new();
    app.set_library_root("/books");

    let mut fs = MockFileSystem::empty();
    fs.add_directory("/");
    fs.add_directory("/books");
    fs.add_file(
        "/books/pg84-frankenstein.epub",
        include_bytes!("../../../sample_books/pg84-frankenstein.epub"),
    );

    ScenarioHarness::new(app, fs)
}

#[test]
fn library_open_frankenstein_scroll_and_capture() {
    let mut harness = setup_harness();
    harness.assert_render_budget_ms(250, "boot");

    assert!(harness.press(Button::Confirm));
    assert_eq!(harness.app().current_screen(), AppScreen::Library);
    assert!(harness.pump_deferred_until_idle() > 0);
    harness.assert_render_budget_ms(250, "library");

    assert!(!harness.press(Button::Confirm));
    assert!(harness.pump_deferred_until_idle() > 0);
    assert_eq!(harness.app().current_screen(), AppScreen::FileBrowser);
    assert!(
        harness.app().file_browser_is_reading_epub(),
        "epub open did not reach reading mode: opening={} status={:?}",
        harness.app().file_browser_is_opening_epub(),
        harness.app().file_browser_status_message()
    );

    assert!(harness.press_and_settle(Button::Right));
    maybe_capture(&harness, "frankenstein_page2");

    let mut visited = BTreeSet::new();
    let mut turns = 0usize;
    let mut validated_back_nav = false;
    const MAX_TURNS: usize = 20000;
    loop {
        let before = harness
            .app()
            .file_browser_epub_position()
            .expect("epub position should exist");
        assert!(
            visited.insert(before),
            "position repeated unexpectedly: {:?}",
            before
        );

        assert!(harness.press_and_settle(Button::Right));
        turns += 1;
        let after = harness
            .app()
            .file_browser_epub_position()
            .expect("epub position should still exist");

        if !validated_back_nav && visited.len() >= 8 {
            let probe_start = after;
            assert!(harness.press_and_settle(Button::Left));
            let probe_back = harness
                .app()
                .file_browser_epub_position()
                .expect("epub position should exist after left probe");
            assert_ne!(
                probe_back, probe_start,
                "left navigation probe should move to previous page"
            );

            assert!(harness.press_and_settle(Button::Right));
            let _probe_forward = harness
                .app()
                .file_browser_epub_position()
                .expect("epub position should exist after right probe");
            validated_back_nav = true;
        }

        if turns == 3 {
            harness.assert_render_budget_ms(250, "frankenstein_page5");
            maybe_capture(&harness, "frankenstein_page5");
        }
        if turns.is_multiple_of(25) {
            harness.assert_render_budget_ms(250, "frankenstein_scroll");
        }

        if after == before {
            break;
        }
        assert!(turns < MAX_TURNS, "hit max turns while scrolling");
    }

    assert!(
        visited.len() >= 8,
        "expected substantial pagination for frankenstein, got {} positions",
        visited.len()
    );
    assert!(validated_back_nav, "did not reach back-nav probe checkpoint");
    harness.assert_render_budget_ms(250, "frankenstein_back_nav");
}

fn maybe_capture(harness: &ScenarioHarness, name: &str) {
    if env::var("SCENARIO_CAPTURE").is_err() {
        return;
    }
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target/scenario-snapshots");
    path.push(format!("{}.png", name));
    harness
        .save_screenshot_png(&path)
        .expect("screenshot capture should succeed");
}
