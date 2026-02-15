use esp_idf_svc::hal::gpio::{Gpio3, Input, PinDriver};
use esp_idf_svc::sys;

use xteink_ui::Button;

const ADC_NO_BUTTON: i32 = 3800;
const ADC_RANGES_1: [i32; 5] = [3800, 3100, 2090, 750, i32::MIN];
const ADC_RANGES_2: [i32; 3] = [3800, 1120, i32::MIN];
const ADC_WIDTH_BIT_12: u32 = 3;
const ADC_ATTEN_DB_11: u32 = 3;

pub fn init_adc() {
    unsafe {
        sys::adc1_config_width(ADC_WIDTH_BIT_12);
        sys::adc1_config_channel_atten(sys::adc_channel_t_ADC_CHANNEL_1, ADC_ATTEN_DB_11);
        sys::adc1_config_channel_atten(sys::adc_channel_t_ADC_CHANNEL_2, ADC_ATTEN_DB_11);
    }
}

pub fn read_adc(channel: sys::adc_channel_t) -> i32 {
    unsafe { sys::adc1_get_raw(channel) as i32 }
}

fn get_button_from_adc(adc_value: i32, ranges: &[i32], num_buttons: usize) -> i32 {
    for i in 0..num_buttons {
        if ranges[i + 1] < adc_value && adc_value <= ranges[i] {
            return i as i32;
        }
    }
    -1
}

pub fn read_buttons(
    power_btn: &mut PinDriver<Gpio3, Input>,
    debug_mode: bool,
) -> (Option<Button>, bool) {
    let power_pressed = power_btn.is_low();
    if power_pressed {
        return (Some(Button::Power), true);
    }

    let adc1_value = read_adc(sys::adc_channel_t_ADC_CHANNEL_1);
    let adc2_value = read_adc(sys::adc_channel_t_ADC_CHANNEL_2);

    if debug_mode && (adc1_value < ADC_NO_BUTTON || adc2_value < ADC_NO_BUTTON) {
        log::info!("ADC1: {}, ADC2: {}", adc1_value, adc2_value);
    }

    let btn1 = get_button_from_adc(adc1_value, &ADC_RANGES_1, 4);
    if btn1 >= 0 {
        return (
            Some(match btn1 {
                0 => Button::Back,
                1 => Button::Confirm,
                2 => Button::Left,
                3 => Button::Right,
                _ => unreachable!(),
            }),
            false,
        );
    }

    let btn2 = get_button_from_adc(adc2_value, &ADC_RANGES_2, 2);
    if btn2 >= 0 {
        return (
            Some(match btn2 {
                0 => Button::VolumeUp,
                1 => Button::VolumeDown,
                _ => unreachable!(),
            }),
            false,
        );
    }

    (None, false)
}
