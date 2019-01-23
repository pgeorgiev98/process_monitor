use std::fs::File;
use std::fs::read_dir;
use std::io::prelude::*;
use std::{thread, time};

struct IoStats {
    total_read_bytes: u64,
    total_write_bytes: u64,
    read_bytes: u64,
    write_bytes: u64,
}

struct Process {
    proc_path: String,
    process_id: i32,
    io_stats: Result<IoStats, &'static str>
}

fn get_io_stats(path: &String) -> Result<IoStats, &'static str> {
    let mut file = match File::open(path) {
        Err(_) => return Err("Failed to open io file"),
        Ok(file) => file,
    };

    let mut contents = String::new();
    if let Err(_) = file.read_to_string(&mut contents) {
        return Err("Failed to read from io file");
    }

    let mut read_bytes : Option<u64> = None;
    let mut write_bytes : Option<u64> = None;
    for line in contents.lines() {
        if line.starts_with("read_bytes: ") {
            if read_bytes != None {
                return Err("Invalid io file format");
            }
            if let Some(bytes) = line.get("read_bytes: ".len()..line.len()) {
                read_bytes = match bytes.parse::<u64>() {
                    Ok(b) => Some(b),
                    Err(_) => return Err("Invalid io file format"),
                }
            } else {
                unreachable!()
            }
        } else if line.starts_with("write_bytes: ") {
            if write_bytes != None {
                return Err("Invalid io file format");
            }
            if let Some(bytes) = line.get("write_bytes: ".len()..line.len()) {
                write_bytes = match bytes.parse::<u64>() {
                    Ok(b) => Some(b),
                    Err(_) => return Err("Invalid io file format"),
                }
            } else {
                unreachable!()
            }
        }
    }

    if let (Some(read), Some(write)) = (read_bytes, write_bytes) {
        Ok(IoStats {
            total_read_bytes: read,
            total_write_bytes: write,
            read_bytes: 0,
            write_bytes: 0,
        })
    } else {
        Err("Invalid io file format")
    }
}

fn refresh_processes(processes: Vec<Process>) -> Vec<Process> {
    let directories = match read_dir("/proc/") {
        Ok(i) => i,
        Err(_) => return Vec::new(),
    };

    let mut new_processes = Vec::new();

    for entry in directories {
        if let Ok(entry) = entry {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    if let Ok(path) = entry.path().into_os_string().into_string() {
                        if let Some(file_name) = entry.path().file_name() {
                            if let Some(file_name) = file_name.to_str() {
                                if let Ok(pid) = file_name.parse::<i32>() {
                                    // TODO: Actually add it even though we can't get the stats
                                    let mut io_stats = get_io_stats(&(path.clone() + "/io"));
                                    if let Ok(io_stats) = &mut io_stats {
                                        for p in &processes {
                                            if p.process_id == pid {
                                                if let Ok(old_io_stats) = &p.io_stats {
                                                    // TODO: check time since last poll
                                                    io_stats.read_bytes = io_stats.total_read_bytes - old_io_stats.total_read_bytes;
                                                    io_stats.write_bytes = io_stats.total_write_bytes - old_io_stats.total_write_bytes;
                                                }
                                                break;
                                            }
                                        }
                                    }
                                    new_processes.push(Process {
                                        proc_path: path,
                                        process_id: pid,
                                        io_stats: io_stats,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    new_processes
}

fn main() {
    let mut processes = Vec::new();
    loop {
        processes = refresh_processes(processes);
        thread::sleep(time::Duration::from_millis(1000));
        println!("");
        for process in &processes {
            println!("{} {} {}",
                     process.proc_path, process.process_id,
                     match &process.io_stats {
                         Ok(s) => format!("r: {}, w: {}", s.read_bytes, s.write_bytes),
                         Err(e) => String::from(*e),
                     });
        }
    }
}
