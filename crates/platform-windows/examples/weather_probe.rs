//! Sonda: uma consulta ao wttr.in pelo HttpClient do Windows.
fn main() {
    let url = zefiro_island::weather::url("");
    match zefiro_platform_windows::http::get_string(&url) {
        Ok(json) => println!("{:?}", zefiro_island::weather::parse(&json, "")),
        Err(e) => println!("erro: {e}"),
    }
}
