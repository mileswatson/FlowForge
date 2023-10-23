use std::io::Result;

fn main() -> Result<()> {
    prost_build::compile_protos(&["src/trainers/remy/dna.proto"], &["src/"])?;
    Ok(())
}
