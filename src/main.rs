fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("snapshot") => println!(
            "{}",
            serde_json::to_string(&kitt_toolbox::snapshot()).expect("serialize snapshot")
        ),
        _ => {
            eprintln!("usage: kitt-toolbox snapshot");
            std::process::exit(2)
        }
    }
}
