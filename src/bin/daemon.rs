use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

use evdev_rs::enums::EV_REL::{REL_X, REL_Y};
use evdev_rs::enums::EV_SYN::{SYN_DROPPED, SYN_REPORT};
use evdev_rs::enums::{EventCode, EventType};
use evdev_rs::{Device, InputEvent, ReadFlag, TimeVal, UInputDevice};

const ACCEL_VALUE: f64 = 0.02;
const ACCEL_POW: f64 = 2.0;
const MOUSE_SENS: f64 = 0.75;
const MOUSE_SENS_CAP: f64 = 1.5;

const SOCKET_PATH: &str = "/run/accel.socket";

fn time_diff(a: &TimeVal, b: &TimeVal) -> f64 {
    let a = a.tv_sec * 1_000_000 + a.tv_usec;
    let b = b.tv_sec * 1_000_000 + b.tv_usec;

    (a - b).abs() as f64 / 1_000f64
}

fn apply_accel(event_values: &mut (i32, i32), time_delta: f64) -> (f64, f64) {
    if time_delta == 0. {
        println!("To close!");
    }

    let (dx, dy) = event_values.clone();
    let vel = (f64::from(dx * dx + dy * dy).sqrt() / time_delta).abs();
    let accel_sens = (MOUSE_SENS + (vel * ACCEL_VALUE).powf(ACCEL_POW - 1.)).min(MOUSE_SENS_CAP);

    *event_values = (
        (dx as f64 * accel_sens).round() as i32,
        (dy as f64 * accel_sens).round() as i32,
    );

    return (vel, accel_sens);
}

fn main() {
    // socket
    if std::fs::metadata(SOCKET_PATH).is_ok() {
        std::fs::remove_file(SOCKET_PATH).unwrap();
    }
    let unix_listener = UnixListener::bind(SOCKET_PATH).unwrap();
    unix_listener.set_nonblocking(true).unwrap(); // así no jode

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

    let mut events = Vec::with_capacity(2);
    loop {
        // primero leemos del socket
        match unix_listener.accept() {
            Ok((mut unix_stream, _)) => {
                let mut message = [0u8; 10];
                let bytes = unix_stream.read(&mut message).unwrap();
                println!("{}", str::from_utf8(&message[..bytes]).unwrap());
            }
            _ => {}
        }

        // "unwrap" bc there is no way this throws an error in blocking mode
        event = mouse.next_event(ReadFlag::BLOCKING).unwrap().1;
        match event.event_code {
            EventCode::EV_REL(REL_X | REL_Y) => {
                events.push(event);
            }

            EventCode::EV_SYN(SYN_REPORT) => {
                // at first, we cant know if events are (dx, dy) or (dy, dx) or if they are at all.
                // and we can't change the order (i.e. recieve (dy, dx) and write (dx, dy)),
                // because mouse movement get's fucked.
                // doesn't matter really, besides having to do some extra checks.

                // early return
                if events.is_empty() {
                    virt.write_event(&event).unwrap();
                    continue;
                }

                // select events and modify
                let x_index = events
                    .iter_mut()
                    .position(|ev| ev.event_code == EventCode::EV_REL(REL_X));

                let y_index = events
                    .iter_mut()
                    .position(|ev| ev.event_code == EventCode::EV_REL(REL_Y));

                let mut values = (
                    x_index.map(|i| events[i].value).unwrap_or(0),
                    y_index.map(|i| events[i].value).unwrap_or(0),
                );

                let _stats = apply_accel(&mut values, time_diff(&event.time, &last_time));

                if let Some(i) = x_index {
                    events[i].value = values.0;
                }
                if let Some(i) = y_index {
                    events[i].value = values.1;
                }

                // write
                for event in &events {
                    virt.write_event(event).unwrap();
                }

                virt.write_event(&event).unwrap();
                events.clear();
                last_time = event.time;
            }

            EventCode::EV_SYN(SYN_DROPPED) => {
                panic!(" --- DROPPED DROPPED DROPPED --- ")
            }

            _ => {
                virt.write_event(&event).unwrap();
            }
        }
    }
}
