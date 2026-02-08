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
        "/books/Fundamental-Accessibility-Tests-Basic-Functionality-v2.0.0.epub",
        include_bytes!(
            "../../../sample_books/Fundamental-Accessibility-Tests-Basic-Functionality-v2.0.0.epub"
        ),
    );

    ScenarioHarness::new(app, fs)
}

#[test]
fn library_open_fundamental_epub_and_scroll_to_end() {
    let mut harness = setup_harness();
    harness.assert_render_budget_ms(250, "boot");

    // System menu -> Library
    assert!(harness.press(Button::Confirm));
    assert_eq!(harness.app().current_screen(), AppScreen::Library);
    assert!(harness.pump_deferred_until_idle() > 0);
    harness.assert_render_budget_ms(250, "library");

    // Open only EPUB entry
    assert!(!harness.press(Button::Confirm));
    assert!(harness.pump_deferred_until_idle() > 0);
    assert_eq!(harness.app().current_screen(), AppScreen::FileBrowser);
    assert!(harness.app().file_browser_is_reading_epub());
    assert!(harness.press(Button::Right));
    maybe_capture(&harness, "fundamental_page2");

    let mut visited = BTreeSet::new();
    let mut turns = 0usize;
    const MAX_TURNS: usize = 5000;

    loop {
        let before = harness
            .app()
            .file_browser_epub_position()
            .expect("epub position should exist while reading");
        assert!(
            visited.insert(before),
            "reader position repeated before reaching end: {:?}",
            before
        );

        harness.assert_render_budget_ms(250, "fundamental_scroll");
        assert!(harness.display().black_pixel_count() > 0);

        assert!(harness.press(Button::Right));
        turns += 1;
        if turns == 2 {
            maybe_capture(&harness, "fundamental_page3");
        }
        let after = harness
            .app()
            .file_browser_epub_position()
            .expect("epub position should still exist");

        if after == before {
            break;
        }

        assert!(
            turns < MAX_TURNS,
            "hit max turns while scrolling EPUB; last position: {:?}",
            after
        );
    }

    assert!(
        visited.len() > 1,
        "expected multiple EPUB pages/positions, visited={}",
        visited.len()
    );

    let final_pos = harness
        .app()
        .file_browser_epub_position()
        .expect("epub position should still exist at end");
    println!(
        "scrolled_epub: positions={} turns={} final_position={:?}",
        visited.len(),
        turns,
        final_pos
    );
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
