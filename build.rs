use vergen::{BuildBuilder, Emitter};

fn main() {
    if let Ok(build) = BuildBuilder::all_build() {
        if let Ok(emitter) = Emitter::default().add_instructions(&build) {
            let _ = emitter.emit();
        }
    }
}
