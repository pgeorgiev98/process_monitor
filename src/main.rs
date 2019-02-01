use std::{thread, time};

mod processes;

fn main() {
    let mut processes = Vec::new();
    loop {
        processes = processes::refresh_processes(processes);
        thread::sleep(time::Duration::from_millis(1000));
        println!("");
        for process in &processes {
            println!("{} {} {}",
                     process.proc_path, process.process_id,
                     match &process.io_stats {
                         Ok(s) => format!("r: {}, w: {}", s.read_bytes, s.write_bytes),
                         Err(e) => e.to_string(),
                     });
        }
    }
}
