//! Sonda: o `http` vai à rede em toda consulta? Duas consultas seguidas à mesma URL pública; a 2ª não
//! pode vir do cache do `HttpClient` (era o que deixava o chip do Claude parado). Sai com 1 se vier.
const URL: &str = "https://api.github.com/repos/Felipe-Cavalari/zefiro/releases/latest";

fn main() {
    let mut cached = false;
    for n in 1..=2 {
        let (status, from_cache) = zefiro_platform_windows::http::get_source(URL).expect("requisição");
        println!("{n}ª: status={status} origem={}", if from_cache { "cache" } else { "rede" });
        cached |= n == 2 && from_cache;
    }
    std::process::exit(i32::from(cached));
}
