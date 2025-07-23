use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};

use evdev_rs::enums::EV_REL::{REL_X, REL_Y};
use evdev_rs::enums::EV_SYN::{SYN_DROPPED, SYN_REPORT};
use evdev_rs::enums::{EventCode, EventType};
use evdev_rs::{Device, InputEvent, ReadFlag, TimeVal, UInputDevice};

const ACCEL_VALUE: f64 = 0.02;
const ACCEL_POW: f64 = 2.0;
const MOUSE_SENS: f64 = 0.5;
const MOUSE_SENS_CAP: f64 = 1.5;

struct MouseMove {
    dx: i32,
    dy: i32,
    time_diff: f64,
}

impl MouseMove {}

const SOCKET_PATH: &str = "/run/accel.socket";

fn time_diff(a: &TimeVal, b: &TimeVal) -> f64 {
    let a = a.tv_sec * 1_000_000 + a.tv_usec;
    let b = b.tv_sec * 1_000_000 + b.tv_usec;

    (a - b).abs() as f64 / 1_000f64
}

struct Events(InputEvent, InputEvent, InputEvent);
fn process_event(event: &mut InputEvent, time_delta: f64) {
    assert!(event.is_type(&EventType::EV_REL));
    if time_delta == 0. {
        println!("To close!");
    }

    // esto está mal, tendría que ser sqrt(dx*dx + dy*dy) / time_delta
    // pero evdev recibe de a un evento el hijo de puta
    let vel = (event.value as f64 / time_delta).abs();
    let accel_sens = (MOUSE_SENS + (vel * ACCEL_VALUE).powf(ACCEL_POW - 1.)).min(MOUSE_SENS_CAP);

    event.value = (event.value as f64 * accel_sens).round() as i32;
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

    let mut events = Vec::with_capacity(3);
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
                events.push(event.clone());
                event = mouse.next_event(ReadFlag::BLOCKING).unwrap().1;
                match event.event_code {
                    EventCode::EV_REL(REL_X | REL_Y) => {
                        events.push(event.clone());
                        event = mouse.next_event(ReadFlag::BLOCKING).unwrap().1;
                        match event.event_code {
                            EventCode::EV_SYN(SYN_REPORT) => {
                                events.push(event.clone());
                            }
                            _ => {
                                virt.write_event(&event).unwrap();
                            }
                        }
                    }
                    EventCode::EV_SYN(SYN_REPORT) => {
                        events.push(event.clone());
                    }
                    _ => {
                        virt.write_event(&event).unwrap();
                    }
                }

                for event in &events {
                    virt.write_event(&event).unwrap();
                }

                events.clear();
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
