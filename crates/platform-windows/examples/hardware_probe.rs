//! Sonda: duas amostras do monitor de hardware.
use std::sync::Arc;
fn main() {
    let m = zefiro_platform_windows::hardware::HardwareMonitor::start(Arc::new(|| {})).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2300));
    println!("{:?}", m.latest());
}
