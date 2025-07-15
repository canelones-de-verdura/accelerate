use std::fs::File;

use evdev_rs::enums::EV_REL::{REL_X, REL_Y};
use evdev_rs::enums::EV_SYN::{SYN_DROPPED, SYN_REPORT};
use evdev_rs::enums::{EventCode, EventType};
use evdev_rs::{Device, InputEvent, ReadFlag, TimeVal, UInputDevice};

const ACCEL_VALUE: f64 = 0.02;
const ACCEL_POW: f64 = 2.0;
const MOUSE_SENS: f64 = 0.75;
const MOUSE_SENS_CAP: f64 = 1.5;

fn time_diff(a: &TimeVal, b: &TimeVal) -> f64 {
    let a = a.tv_sec * 1_000_000 + a.tv_usec;
    let b = b.tv_sec * 1_000_000 + b.tv_usec;

    (a - b).abs() as f64 / 1_000f64
}

fn process_event(event: &mut InputEvent, time_delta: f64) {
    assert!(event.is_type(&EventType::EV_REL));
    if time_delta == 0. {
        println!("To close!");
    }

    let vel = (event.value as f64 / time_delta).abs();
    let accel_sens = (MOUSE_SENS + (vel * ACCEL_VALUE).powf(ACCEL_POW - 1.)).min(MOUSE_SENS_CAP);

    event.value = (event.value as f64 * accel_sens).round() as i32;
}

fn main() {
    // entiendo que por default /dev/input/event5 es el mouse
    let event_num = 5;
    let fd = File::open(format!("/dev/input/event{event_num}")).unwrap();
    let mut mouse = Device::new_from_file(fd).unwrap();

    mouse.grab(evdev_rs::GrabMode::Grab).unwrap();

    let virt = UInputDevice::create_from_device(&mouse).unwrap();

    println!("running");

    let mut event;
    let mut last_time = TimeVal {
        tv_sec: 0,
        tv_usec: 0,
    };

    loop {
        // "unwrap" bc there is no way this throws an error in blocking mode
        event = mouse.next_event(ReadFlag::BLOCKING).unwrap().1;
        match event.event_code {
            EventCode::EV_REL(REL_X | REL_Y) => {
                let delta = time_diff(&event.time, &last_time);
                process_event(&mut event, delta);
            }
            EventCode::EV_SYN(SYN_REPORT) => {
                last_time = event.time;
            }
            EventCode::EV_SYN(SYN_DROPPED) => {
                panic!(" --- DROPPED DROPPED DROPPED --- ")
            }
            _ => {}
        }

        virt.write_event(&event).unwrap();
    }
}
