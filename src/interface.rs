extern crate gtk;

use gtk::prelude::*;

use gtk::{Window, WindowType};
use gtk::{TreeView, ListStore, TreeIter, TreeViewColumn, CellRendererText};
use gtk::ScrolledWindow;
use gtk::{Box, Orientation};
use gtk::Statusbar;

#[path = "processes.rs"] mod processes;
use processes::{ProcessesList, Process};

use std::rc::Rc;
use std::cell::RefCell;

pub struct Interface {
    process_view: Rc<RefCell<ProcessView>>,
}

struct ProcessView {
    processes: ProcessesList,
    model: ListStore,
    tree: TreeView,
    processes_in_model: Vec<(u64, TreeIter)>,
    default_background_color: (f64, f64, f64),
}

impl Interface {
    pub fn new() -> Result<Interface, &'static str> {
        if gtk::init().is_err() {
            return Err("Failed to initialize GTK");
        }

        let window = Window::new(WindowType::Toplevel);
        window.set_title("Process Monitor");
        window.set_default_size(640, 480);

        let statusbar = Statusbar::new();
        statusbar.push(0, "");

        let process_view = Rc::new(RefCell::new(ProcessView::new()));

        let scrolled_window = ScrolledWindow::new(None, None);

        scrolled_window.add(&process_view.borrow_mut().tree);

        let main_box = Box::new(Orientation::Vertical, 0);
        main_box.pack_start(&scrolled_window, true, true, 0);
        main_box.pack_start(&statusbar, false, false, 0);
        window.add(&main_box);
        window.show_all();

        window.connect_delete_event(|_, _| {
            gtk::main_quit();
            Inhibit(false)
        });

        process_view.borrow_mut().refresh();

        let process_view_clone = process_view.clone();
        timeout_add(1000, move || {
            process_view_clone.borrow_mut().refresh();
            statusbar.pop(0);
            let disk_stats = &process_view_clone.borrow_mut().processes.disk_stats;
            statusbar.push(0, format!("Read: {: >12}                Write: {: >12}",
                                      format_bytes_per_second(disk_stats.maximum_read),
                                      format_bytes_per_second(disk_stats.maximum_write)).as_str());
            Continue(true)
        });

        Ok(Interface { process_view })
    }

    pub fn exec() {
        gtk::main();
    }
}

impl ProcessView {
    fn new() -> ProcessView {
        let model = ListStore::new(&[u64::static_type(), String::static_type(), String::static_type(), String::static_type(), String::static_type(), String::static_type()]);
        let tree = TreeView::new();

        append_column(&tree, 0, "PID", None);
        append_column(&tree, 1, "Process Name", None);
        append_column(&tree, 2, "Read Bytes", Some(4));
        append_column(&tree, 3, "Write Bytes", Some(5));

        tree.set_model(Some(&model));

        for (i, column) in tree.get_columns().iter().enumerate() {
            column.set_sort_column_id(i as i32);
        }

        let default_background_color = match tree.get_style_context() {
            Some(context) => {
                let col = context.get_background_color(gtk::StateFlags::ACTIVE);
                (col.red, col.green, col.blue)
            },
            None => (1.0, 1.0, 1.0),
        };

        ProcessView {
            processes: ProcessesList::new(),
            model: model,
            tree: tree,
            processes_in_model: Vec::new(),
            default_background_color,
        }
    }

    fn refresh(&mut self) {
        self.processes = processes::refresh_processes(&&self.processes);

        let mut new_processes_in_model = Vec::new();

        for process in &self.processes.processes {
            if let Some((_, iter)) = self.processes_in_model.iter().find(|(pid, _)| { *pid == process.pid }) {
                self.set_process(&iter, process);
                new_processes_in_model.push((process.pid, iter.clone()));
            } else {
                let iter = self.append_process(process);
                new_processes_in_model.push((process.pid, iter));
            }
        }

        for p in &self.processes_in_model {
            if new_processes_in_model.iter().find(|(pid, _)| { *pid == p.0 }) == None {
                self.model.remove(&p.1);
            }
        }

        self.processes_in_model = new_processes_in_model;
    }

    fn set_process(&self, iter: &TreeIter, process: &Process) {
        let (read_bytes, write_bytes, read_coef, write_coef) = match &process.io_stats {
            Ok(s) => (
                format_bytes_per_second(s.read_bytes),
                format_bytes_per_second(s.write_bytes),
                if self.processes.disk_stats.maximum_read > 0 {
                    (s.read_bytes as f64) / (self.processes.disk_stats.maximum_read as f64)
                } else {
                    0.0
                },
                if self.processes.disk_stats.maximum_write > 0 {
                    (s.write_bytes as f64) / (self.processes.disk_stats.maximum_write as f64)
                } else {
                    0.0
                }
            ),
            Err(_) => (String::from("-"), String::from("-"), 0.0, 0.0),
        };
        let default_name = String::from("?");
        let name = match &process.name {
            Ok(n) => n,
            Err(_) => &default_name,
        };

        let bgr = format!("#{:02X}{:02X}{:02X}",
                          (read_coef * 255.0 + (1.0 - read_coef) * self.default_background_color.0 * 255.0) as i32,
                          (read_coef *  17.0 + (1.0 - read_coef) * self.default_background_color.1 * 255.0) as i32,
                          (read_coef *  51.0 + (1.0 - read_coef) * self.default_background_color.2 * 255.0) as i32);
        let bgw = format!("#{:02X}{:02X}{:02X}",
                          (write_coef * 255.0 + (1.0 - write_coef) * self.default_background_color.0 * 255.0) as i32,
                          (write_coef *  17.0 + (1.0 - write_coef) * self.default_background_color.1 * 255.0) as i32,
                          (write_coef *  51.0 + (1.0 - write_coef) * self.default_background_color.2 * 255.0) as i32);

        self.model.set(&iter, &[0, 1, 2, 3, 4, 5], &[&process.pid, &name, &read_bytes, &write_bytes, &bgr, &bgw]);
    }

    fn append_process(&self, process: &Process) -> TreeIter {
        let (read_bytes, write_bytes, read_coef, write_coef) = match &process.io_stats {
            Ok(s) => (
                format_bytes_per_second(s.read_bytes),
                format_bytes_per_second(s.write_bytes),
                if self.processes.disk_stats.maximum_read > 0 {
                    (s.read_bytes as f64) / (self.processes.disk_stats.maximum_read as f64)
                } else {
                    0.0
                },
                if self.processes.disk_stats.maximum_write > 0 {
                    (s.write_bytes as f64) / (self.processes.disk_stats.maximum_write as f64)
                } else {
                    0.0
                }
            ),
            Err(_) => (String::from("-"), String::from("-"), 0.0, 0.0),
        };
        let default_name = String::from("?");
        let name = match &process.name {
            Ok(n) => n,
            Err(_) => &default_name,
        };

        let bgr = format!("#{:02X}{:02X}{:02X}",
                          (read_coef * 255.0 + (1.0 - read_coef) * self.default_background_color.0 * 255.0) as i32,
                          (read_coef *  17.0 + (1.0 - read_coef) * self.default_background_color.1 * 255.0) as i32,
                          (read_coef *  51.0 + (1.0 - read_coef) * self.default_background_color.2 * 255.0) as i32);
        let bgw = format!("#{:02X}{:02X}{:02X}",
                          (write_coef * 255.0 + (1.0 - write_coef) * self.default_background_color.0 * 255.0) as i32,
                          (write_coef *  17.0 + (1.0 - write_coef) * self.default_background_color.1 * 255.0) as i32,
                          (write_coef *  51.0 + (1.0 - write_coef) * self.default_background_color.2 * 255.0) as i32);

        self.model.insert_with_values(None, &[0, 1, 2, 3, 4, 5], &[&process.pid, &name, &read_bytes, &write_bytes, &bgr, &bgw])
    }
}


fn append_column(tree: &TreeView, id: i32, title: &'static str, background_id: Option<i32>) {
    let column = TreeViewColumn::new();
    let cell = CellRendererText::new();

    column.set_title(title);

    column.pack_start(&cell, true);
    // Association of the view's column with the model's `id` column.
    column.add_attribute(&cell, "text", id);
    if let Some(background_id) = background_id {
        column.add_attribute(&cell, "background", background_id);
    }
    tree.append_column(&column);
}

fn format_bytes_per_second(bytes: u64) -> String {
    let table = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB"];
    let mut i = 0;
    let mut b: f64 = bytes as f64;
    while b >= 2048.0 {
        b /= 1024.0;
        i += 1;
    }

    format!("{:.1} {}/s", b, table[i])
}
