use std::io::Result;

fn main() -> Result<()> {
    protobuf_codegen::Codegen::new()
        .pure()
        // All inputs and imports from the inputs must reside in `includes` directories.
        .includes(["src/protocols/remy"])
        // Inputs must reside in some of include paths.
        .input("src/protocols/remy/remy_dna.proto")
        // Specify output directory relative to Cargo output directory.
        .cargo_out_dir("protos")
        .run_from_script();
    Ok(())
}
