use std::{
    ffi::{c_char, CStr},
    path::Path,
};

use flowforge::{
    protocols::{
        remy::{dna::RemyDna, point::Point, rule_tree::RuleTree},
        remyr::dna::RemyrDna,
    },
    quantities::{milliseconds, Time},
    Config, Custom,
};
enum Dna {
    Remy(RemyDna),
    Remyr(RemyrDna),
}

#[repr(C)]
struct CAction {
    new_window: u32,
    intersend_seconds: f64,
}

#[no_mangle]
unsafe extern "C" fn load_dna(path: *const c_char) -> *mut Dna {
    let path = Path::new(unsafe { CStr::from_ptr(path) }.to_str().unwrap());
    let d = if path.ends_with(".remyr.dna") {
        Dna::Remyr(<RemyrDna as Config<Custom>>::load(path).unwrap())
    } else {
        Dna::Remy(RemyDna::load(path).unwrap())
    };
    Box::into_raw(Box::new(d))
}

#[no_mangle]
unsafe extern "C" fn free_dna(dna: *mut Dna) {
    unsafe { drop(Box::from_raw(dna)) }
}

#[no_mangle]
unsafe extern "C" fn get_action(
    dna: *mut Dna,
    ack_ewma_ms: f64,
    send_ewma_ms: f64,
    rtt_ratio: f64,
    current_window: u32,
) -> CAction {
    let dna = unsafe { Box::from_raw(dna) };
    let point = Point {
        ack_ewma: milliseconds(ack_ewma_ms),
        send_ewma: milliseconds(send_ewma_ms),
        rtt_ratio,
    };
    let tree: &dyn RuleTree = match &*dna {
        Dna::Remy(d) => d,
        Dna::Remyr(d) => d,
    };

    let action = tree.action(&point, Time::SIM_START).unwrap();
    CAction {
        new_window: action.apply_to(current_window),
        intersend_seconds: action.intersend_delay.seconds(),
    }
}
