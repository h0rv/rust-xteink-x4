use xteink_scenario_harness::ScenarioHarness;
use xteink_ui::{App, AppScreen, Button, MockFileSystem};

fn setup_harness() -> ScenarioHarness {
    let app = App::new();

    let mut fs = MockFileSystem::empty();
    fs.add_directory("/");
    fs.add_directory("/books");
    fs.add_file(
        "/books/sample.txt",
        b"Sample text content.\nThis is a test file.\nThird line.",
    );

    ScenarioHarness::new(app, fs)
}

#[test]
fn screenshot_all_ui_screens() {
    let mut harness = setup_harness();

    // System Menu
    harness.render();
    harness
        .save_screenshot_png("screenshots/01_system_menu.png")
        .expect("Failed to save system menu screenshot");

    // Library
    harness.press(Button::Confirm); // Select Library (first item)
    harness.pump_deferred_until_idle();
    harness.render();
    harness
        .save_screenshot_png("screenshots/02_library.png")
        .expect("Failed to save library screenshot");

    // Back to System Menu
    harness.press(Button::Back);
    harness.render();

    // Files
    harness.press(Button::Aux2); // Move to Files
    harness.press(Button::Confirm);
    harness.pump_deferred_until_idle();
    harness.render();
    harness
        .save_screenshot_png("screenshots/03_files.png")
        .expect("Failed to save files screenshot");

    // Back to System Menu
    harness.press(Button::Back);
    harness.render();

    // Reader Settings
    harness.press(Button::Aux2); // Move to Reader Settings
    harness.press(Button::Confirm);
    harness.pump_deferred_until_idle();
    harness.render();
    harness
        .save_screenshot_png("screenshots/04_reader_settings.png")
        .expect("Failed to save reader settings screenshot");

    // Back to System Menu
    harness.press(Button::Back);
    harness.render();

    // Device Settings
    harness.press(Button::Aux2); // Move to Device Settings
    harness.press(Button::Confirm);
    harness.pump_deferred_until_idle();
    harness.render();
    harness
        .save_screenshot_png("screenshots/05_device_settings.png")
        .expect("Failed to save device settings screenshot");

    // Back to System Menu
    harness.press(Button::Back);
    harness.render();

    // Information
    harness.press(Button::Aux2); // Move to Information
    harness.press(Button::Confirm);
    harness.pump_deferred_until_idle();
    harness.render();
    harness
        .save_screenshot_png("screenshots/06_information.png")
        .expect("Failed to save information screenshot");

    // Back to System Menu
    harness.press(Button::Back);
    harness.render();

    // Sleep modal
    harness.press(Button::Aux2); // Move to Sleep
    harness.press(Button::Confirm);
    harness.render();
    harness
        .save_screenshot_png("screenshots/07_sleep_modal.png")
        .expect("Failed to save sleep modal screenshot");

    // Cancel modal
    harness.press(Button::Back);
    harness.render();

    // Power Off modal
    harness.press(Button::Aux2); // Move to Power Off
    harness.press(Button::Confirm);
    harness.render();
    harness
        .save_screenshot_png("screenshots/08_power_off_modal.png")
        .expect("Failed to save power off modal screenshot");
}
