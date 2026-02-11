use std::thread;
use std::{env, path::PathBuf};

use xteink_scenario_harness::ScenarioHarness;
use xteink_ui::{App, AppScreen, Button, MockFileSystem};

fn setup_harness() -> ScenarioHarness {
    let mut app = App::new();
    app.set_library_root("/books");

    let mut fs = MockFileSystem::empty();
    fs.add_directory("/");
    fs.add_directory("/books");
    fs.add_file(
        "/books/sample.epub",
        include_bytes!("../../../sample_books/sample.epub"),
    );

    ScenarioHarness::new(app, fs)
}

fn run_library_open_epub_render_back_flow(enable_capture: bool, turn_page: bool) {
    let mut harness = setup_harness();

    harness.render();

    // System menu -> Library
    assert!(harness.press(Button::Confirm));
    assert_eq!(harness.app().current_screen(), AppScreen::Library);

    let scan_updates = harness.pump_deferred_until_idle();
    assert!(
        scan_updates > 0,
        "library scan should produce deferred updates"
    );

    harness.render();

    // Open selected EPUB from library.
    assert!(!harness.press(Button::Confirm));

    let open_updates = harness.pump_deferred_until_idle();
    assert!(
        open_updates > 0,
        "open flow should produce deferred updates"
    );
    assert_eq!(harness.app().current_screen(), AppScreen::FileBrowser);
    assert!(
        harness.app().file_browser_is_reading_epub(),
        "epub open did not reach reading mode: opening={} status={:?}",
        harness.app().file_browser_is_opening_epub(),
        harness.app().file_browser_status_message()
    );

    harness.render();
    assert!(harness.display().black_pixel_count() > 0);
    maybe_capture(&harness, "library_epub_page1", enable_capture);

    if turn_page {
        // Turn a page to help diagnose layout pagination issues.
        assert!(harness.press(Button::Right));
        harness.render();
        assert!(harness.display().black_pixel_count() > 0);
        maybe_capture(&harness, "library_epub_page2", enable_capture);
    }

    // Back should return to library when opened from library.
    assert!(harness.press(Button::Back));
    assert_eq!(harness.app().current_screen(), AppScreen::Library);
}

fn maybe_capture(harness: &ScenarioHarness, name: &str, enabled: bool) {
    if !enabled || env::var("SCENARIO_CAPTURE").is_err() {
        return;
    }
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target/scenario-snapshots");
    path.push(format!("{}.png", name));
    harness
        .save_screenshot_png(&path)
        .expect("screenshot capture should succeed");
}

#[test]
fn library_open_epub_render_back_flow() {
    run_library_open_epub_render_back_flow(true, true);
}

#[test]
fn library_open_epub_render_back_flow_small_stack_thread() {
    let handle = thread::Builder::new()
        .name("scenario-small-stack".to_string())
        .stack_size(384 * 1024)
        .spawn(|| run_library_open_epub_render_back_flow(false, false))
        .expect("thread spawn should succeed");

    handle.join().expect("small-stack scenario should pass");
}
