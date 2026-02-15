use xteink_scenario_harness::ScenarioHarness;
use xteink_ui::{App, Button, MockFileSystem};

#[test]
fn screenshot_modals() {
    let app = App::new();
    let fs = MockFileSystem::empty();
    let mut harness = ScenarioHarness::new(app, fs);

    // Navigate to Sleep modal
    for _ in 0..5 {
        harness.press(Button::VolumeDown);
    }
    harness.press(Button::Confirm); // Open Sleep modal
    harness.render();
    harness
        .save_screenshot_png("screenshots/modal_sleep.png")
        .expect("Failed to save sleep modal");

    // Navigate to the confirm button in modal
    harness.press(Button::Right);
    harness.render();
    harness
        .save_screenshot_png("screenshots/modal_sleep_confirm_selected.png")
        .expect("Failed to save sleep modal with confirm selected");

    // Cancel and go to Power Off
    harness.press(Button::Back);
    harness.press(Button::VolumeDown);
    harness.press(Button::Confirm); // Open Power Off modal
    harness.render();
    harness
        .save_screenshot_png("screenshots/modal_poweroff.png")
        .expect("Failed to save power off modal");
}
