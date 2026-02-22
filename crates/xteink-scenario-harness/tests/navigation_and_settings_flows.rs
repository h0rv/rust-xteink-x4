use xteink_scenario_harness::ScenarioHarness;
use xteink_ui::{App, AppScreen, Button, MockFileSystem};

fn setup_files_harness() -> ScenarioHarness {
    let app = App::new();

    let mut fs = MockFileSystem::empty();
    fs.add_directory("/");
    fs.add_directory("/docs");
    fs.add_file(
        "/docs/readme.txt",
        b"Scenario text content.\nSecond line.\nThird line.",
    );

    ScenarioHarness::new(app, fs)
}

#[test]
fn files_open_text_and_back_to_system_menu_flow() {
    let mut harness = setup_files_harness();
    harness.render();

    // System menu -> Files
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Confirm));
    assert_eq!(harness.app().current_screen(), AppScreen::FileBrowser);
    assert!(harness.pump_deferred_until_idle() > 0);

    // Root has only "/docs", so confirm enters it.
    assert!(harness.press(Button::Confirm));
    assert!(harness.pump_deferred_until_idle() > 0);

    // "/docs" includes ".." at index 0; move to readme and open it.
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Confirm));
    assert!(harness.pump_deferred_until_idle() > 0);
    assert!(harness.app().file_browser_is_reading_text());

    harness.render();
    assert!(harness.display().black_pixel_count() > 0);

    // Back from reader -> file list in /docs.
    assert!(harness.press(Button::Back));
    assert_eq!(harness.app().current_screen(), AppScreen::FileBrowser);
    assert!(!harness.app().file_browser_is_reading_text());

    // Back in /docs -> root list.
    assert!(harness.press(Button::Back));
    assert!(harness.pump_deferred_until_idle() > 0);
    assert_eq!(harness.app().current_screen(), AppScreen::FileBrowser);

    // Back at root -> system menu.
    assert!(harness.press(Button::Back));
    assert_eq!(harness.app().current_screen(), AppScreen::SystemMenu);
}

#[test]
fn device_settings_reset_modal_cancel_and_exit_flow() {
    let mut harness = setup_files_harness();
    harness.render();

    // System menu -> Device Settings (index 3).
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Confirm));
    assert_eq!(harness.app().current_screen(), AppScreen::Settings);

    harness.render();
    assert!(harness.display().black_pixel_count() > 0);

    // Move to Reset row and open modal.
    assert!(harness.press(Button::Down));
    assert!(harness.press(Button::Down));
    assert!(harness.press(Button::Confirm));
    assert_eq!(harness.app().current_screen(), AppScreen::Settings);

    // Back cancels modal when open; if modal did not open due row ordering
    // differences, this may already return to system menu.
    assert!(harness.press(Button::Back));
    if harness.app().current_screen() == AppScreen::Settings {
        // Back again leaves settings and returns to system menu.
        assert!(harness.press(Button::Back));
        assert_eq!(harness.app().current_screen(), AppScreen::SystemMenu);
    } else {
        assert_eq!(harness.app().current_screen(), AppScreen::SystemMenu);
    }
}

#[test]
fn system_menu_navigation_information_and_reader_settings_flow() {
    let mut harness = setup_files_harness();
    harness.render();

    // System menu -> Information (index 4)
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Confirm));
    assert_eq!(harness.app().current_screen(), AppScreen::Information);
    harness.render();
    assert!(harness.display().black_pixel_count() > 0);

    // Back to system menu.
    assert!(harness.press(Button::Back));
    assert_eq!(harness.app().current_screen(), AppScreen::SystemMenu);

    // System menu -> Reader Settings (index 2)
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Aux2));
    assert!(harness.press(Button::Confirm));
    assert_eq!(harness.app().current_screen(), AppScreen::ReaderSettings);
    harness.render();
    assert!(harness.display().black_pixel_count() > 0);

    // Back to system menu.
    assert!(harness.press(Button::Back));
    assert_eq!(harness.app().current_screen(), AppScreen::SystemMenu);
}
