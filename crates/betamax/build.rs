use vergen::Emitter;

fn main() -> anyhow::Result<()> {
    Emitter::default().emit()
}
