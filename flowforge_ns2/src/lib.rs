use flowforge::quantities::Float;

#[no_mangle]
pub extern "C" fn rust_function() -> Float {
    println!("called!");
    let x: Float = 5.;
    x
}
