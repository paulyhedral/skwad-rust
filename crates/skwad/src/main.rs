use skwad_core::t;

fn main() {
    #[cfg(feature = "gui")]
    {
        run_gui();
    }

    #[cfg(not(feature = "gui"))]
    {
        eprintln!("{}: gui feature not built", t("app.name"));
    }
}

#[cfg(feature = "gui")]
fn run_gui() {
    unimplemented!("gpui window shell: rust-port-foundation task 1.5 follow-up");
}
