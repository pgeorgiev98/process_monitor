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
    pub pid: u64,
    pub name: Result<String, Error>,
    pub io_stats: Result<IoStats, Error>,
}

pub struct DiskStats {
    pub total_read: u64,
    pub total_write: u64,
    pub maximum_read: u64,
    pub maximum_write: u64,
}

pub struct ProcessesList {
    pub processes: Vec<Process>,
    pub disk_stats: DiskStats,
}

impl ProcessesList {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            disk_stats: DiskStats {
                total_read: 0,
                total_write: 0,
                maximum_read: 0,
                maximum_write: 0,
            },
        }
    }
}

pub fn refresh_processes(processes_list: &ProcessesList) -> ProcessesList {
    let directories = match read_dir("/proc/") {
        Ok(i) => i,
        Err(_) => return ProcessesList::new(),
    };

    let mut new_processes = ProcessesList::new();

    for entry in directories {
        if let Ok(entry) = entry {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    if let Ok(path) = entry.path().into_os_string().into_string() {
                        if let Some(file_name) = entry.path().file_name() {
                            if let Some(file_name) = file_name.to_str() {
                                if let Ok(pid) = file_name.parse::<u64>() {
                                    // TODO: Actually add it even though we can't get the stats
                                    let mut io_stats = get_io_stats(&path);
                                    if let Ok(io_stats) = &mut io_stats {
                                        for p in &processes_list.processes {
                                            if p.pid == pid {
                                                if let Ok(old_io_stats) = &p.io_stats {
                                                    // TODO: check time since last poll
                                                    io_stats.read_bytes = io_stats.total_read_bytes - old_io_stats.total_read_bytes;
                                                    io_stats.write_bytes = io_stats.total_write_bytes - old_io_stats.total_write_bytes;
                                                }
                                                break;
                                            }
                                        }
                                    }
                                    new_processes.processes.push(Process {
                                        pid: pid,
                                        name: get_process_name(&path),
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

    let current_disk_stats = {
        let mut stats = DiskStats {
            total_read: 0,
            total_write: 0,
            maximum_read: 0,
            maximum_write: 0,
        };

        use std::cmp::max;
        for process in &new_processes.processes {
            if let Ok(io_stats) = &process.io_stats {
                let read = io_stats.read_bytes;
                let write = io_stats.write_bytes;
                stats.total_read += read;
                stats.total_write += write;
                stats.maximum_read = max(stats.maximum_read, read);
                stats.maximum_write = max(stats.maximum_write, write);
            }
        }

        stats
    };
    new_processes.disk_stats = current_disk_stats;

    new_processes
}

fn get_io_stats(proc_path: &String) -> Result<IoStats, Error> {
    let mut file = File::open(proc_path.clone() + "/io")?;

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

fn get_process_name(proc_path: &String) -> Result<String, Error> {
    let mut file = File::open(proc_path.clone() + "/comm")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(String::from(contents.trim()))
}
