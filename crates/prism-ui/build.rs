fn main() {
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=../../assets/icons");
    slint_build::compile("ui/app.slint").expect("compile Prism Shell UI");
}
