fn main() {
    // Styl "fluent" (výchozí ovládací prvky); barvy vlastních prvků jsou v ui/theme.slint.
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/main.slint", config).expect("chyba v ui/*.slint");
    println!("cargo:rerun-if-changed=ui");

    // Ikona a údaje o verzi ve výsledném .exe (Windows).
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/timetrek.ico");
        res.set("ProductName", "TimeTrek");
        res.set("FileDescription", "TimeTrek");
        res.compile().expect("nelze vložit zdroje do .exe");
    }
}
