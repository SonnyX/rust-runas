use runas;

fn main() {
    println!("Running id as root:");
    println!(
        "Status: {}",
        runas::Command::new("touch")
            .arg("/tmp/test.foo")
            .disable_force_prompt()
            .status()
            .expect("failed to wait on child")
    );
}
