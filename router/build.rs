fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS")
        .map(|os| os != "windows")
        .unwrap_or(true)
    {
        return;
    }

    println!("cargo:rerun-if-changed=../assets/icons/router.ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon("../assets/icons/router.ico");
    res.compile()
        .expect("failed to embed router Windows resources");
}
