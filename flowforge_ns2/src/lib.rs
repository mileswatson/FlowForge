use flowforge::quantities::Float;

use dfdx::prelude::*;

#[no_mangle]
pub extern "C" fn rust_function() -> Float {
    let device = Cpu::default();

    let x = device.tensor([[1., 0.], [0., 1.]]).matmul(device.tensor([[0., -1.], [1.,0.]]));

    println!("{:?}", x.array());

    println!("called!");
    let x: Float = 5.;
    x
}
