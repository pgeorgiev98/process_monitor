use std::fs::File;
use std::io::Read;
use std::fs::read_dir;
use std::io::{Error, ErrorKind};

pub struct IoStats {
    pub total_read_bytes: u64,
    pub total_write_bytes: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

pub struct Process {
    pub proc_path: String,
    pub process_id: i32,
    pub io_stats: Result<IoStats, Error>,
}

pub fn refresh_processes(processes: Vec<Process>) -> Vec<Process> {
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

fn get_io_stats(path: &String) -> Result<IoStats, Error> {
    let mut file = File::open(path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let mut read_bytes : Option<u64> = None;
    let mut write_bytes : Option<u64> = None;
    let file_format_error = Err(Error::new(ErrorKind::Other, "Invalid io file format"));
    for line in contents.lines() {
        if line.starts_with("read_bytes: ") {
            if read_bytes != None {
                return file_format_error;
            }
            if let Some(bytes) = line.get("read_bytes: ".len()..line.len()) {
                read_bytes = match bytes.parse::<u64>() {
                    Ok(b) => Some(b),
                    Err(_) => return file_format_error,
                }
            } else {
                unreachable!()
            }
        } else if line.starts_with("write_bytes: ") {
            if write_bytes != None {
                return file_format_error;
            }
            if let Some(bytes) = line.get("write_bytes: ".len()..line.len()) {
                write_bytes = match bytes.parse::<u64>() {
                    Ok(b) => Some(b),
                    Err(_) => return file_format_error,
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
        file_format_error
    }
}