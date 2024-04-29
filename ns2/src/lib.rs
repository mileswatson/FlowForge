use std::{
    ffi::{c_char, c_double, c_uint, CStr},
    path::Path,
};

use flowforge::{
    ccas::{
        remy::{dna::RemyDna, point::Point, RemyPolicy},
        remyr::dna::RemyrDna,
    },
    quantities::milliseconds,
    Config, Custom,
};

#[repr(C)]
struct CAction {
    new_window: c_uint,
    intersend_seconds: c_double,
}

#[no_mangle]
unsafe extern "C" fn load_dna(path: *const c_char) -> *mut Box<dyn RemyPolicy> {
    let path = unsafe { CStr::from_ptr(path) }.to_str().unwrap();
    let is_remyr = path.contains(".remyr.dna");
    let path = Path::new(path);
    let d: Box<dyn RemyPolicy> = if is_remyr {
        Box::new(<RemyrDna as Config<Custom>>::load(path).unwrap())
    } else {
        Box::new(RemyDna::load(path).unwrap())
    };
    let p = Box::into_raw(Box::new(d));
    println!("Loaded dna {:?}...", p);
    p
}

#[no_mangle]
unsafe extern "C" fn free_dna(dna: *mut Box<dyn RemyPolicy>) {
    println!("Freeing {:?}...", dna);
    unsafe { drop(Box::from_raw(dna)) }
}

#[no_mangle]
unsafe extern "C" fn get_action(
    dna: *const Box<dyn RemyPolicy>,
    ack_ewma_ms: c_double,
    send_ewma_ms: c_double,
    rtt_ratio: c_double,
    current_window: c_uint,
) -> CAction {
    let dna = unsafe { dna.as_ref().unwrap() };
    let point = Point {
        ack_ewma: milliseconds(ack_ewma_ms),
        send_ewma: milliseconds(send_ewma_ms),
        rtt_ratio,
    };

    let action = dna.action(&point).unwrap();

    CAction {
        new_window: action.apply_to(current_window),
        intersend_seconds: action.intersend_delay.seconds(),
    }
}
