extern crate gtk;
extern crate nix;

use gtk::prelude::*;

use gtk::{Window, WindowType};
use gtk::{TreeView, ListStore, TreeIter, TreeViewColumn, CellRendererText};
use gtk::ScrolledWindow;
use gtk::{Box, Orientation};
use gtk::Statusbar;
use gtk::{Menu, MenuItem};

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
    processes_in_model: Vec<(i32, TreeIter)>,
    default_background_color: (f64, f64, f64),
}

enum Column {
    PID = 0,
    Name = 1,
    ReadBytes = 2,
    WriteBytes = 3,
    ReadBytesRaw = 4,
    WriteBytesRaw = 5,
    ReadBytesColor = 6,
    WriteBytesColor = 7,
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
        let model = ListStore::new(
            &[
            i32::static_type(),
            String::static_type(),
            String::static_type(),
            String::static_type(),
            u64::static_type(),
            u64::static_type(),
            String::static_type(),
            String::static_type(),
            ]);
        let tree = TreeView::new();

        append_column(&tree, Column::PID as i32, "PID", None);
        append_column(&tree, Column::Name as i32, "Process Name", None);
        append_column(&tree, Column::ReadBytes as i32, "Read Bytes", Some(Column::ReadBytesColor as i32));
        append_column(&tree, Column::WriteBytes as i32, "Write Bytes", Some(Column::WriteBytesColor as i32));

        tree.set_model(Some(&model));

        let columns = tree.get_columns();
        columns[Column::PID as usize].set_sort_column_id(Column::PID as i32);
        columns[Column::Name as usize].set_sort_column_id(Column::Name as i32);
        columns[Column::ReadBytes as usize].set_sort_column_id(Column::ReadBytesRaw as i32);
        columns[Column::WriteBytes as usize].set_sort_column_id(Column::WriteBytesRaw as i32);

        let default_background_color = match tree.get_style_context() {
            Some(context) => {
                let col = context.get_background_color(gtk::StateFlags::ACTIVE);
                (col.red, col.green, col.blue)
            },
            None => (1.0, 1.0, 1.0),
        };

        let menu_popup = Menu::new();

        let menu_kill_item = MenuItem::new_with_mnemonic("_Kill");
        menu_popup.append(&menu_kill_item);

        let menu_terminate_item = MenuItem::new_with_mnemonic("_Terminate");
        menu_popup.append(&menu_terminate_item);

        menu_popup.show_all();

        let tree_copy = tree.clone();
        menu_kill_item.connect_activate(move |_| {
            send_signal_to_selected(&tree_copy, nix::sys::signal::Signal::SIGKILL);
        });
        let tree_copy = tree.clone();
        menu_terminate_item.connect_activate(move |_| {
            send_signal_to_selected(&tree_copy, nix::sys::signal::Signal::SIGTERM);
        });

        tree.connect_button_press_event(move |_, button_event| {
            if button_event.get_button() == 3 {
                menu_popup.popup_at_pointer(None);
            }
            Inhibit(false)
        });

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
        let (read_bytes, write_bytes, read_bytes_string, write_bytes_string, read_coef, write_coef) = match &process.io_stats {
            Ok(s) => (
                s.read_bytes,
                s.write_bytes,
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
            Err(_) => (0, 0, String::from("-"), String::from("-"), 0.0, 0.0),
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

        self.model.set(
            &iter,
            &[
                Column::PID as u32, Column::Name as u32,
                Column::ReadBytes as u32, Column::WriteBytes as u32,
                Column::ReadBytesRaw as u32, Column::WriteBytesRaw as u32,
                Column::ReadBytesColor as u32, Column::WriteBytesColor as u32,
            ],
            &[&process.pid, &name, &read_bytes_string, &write_bytes_string, &read_bytes, &write_bytes, &bgr, &bgw],
            );
    }

    fn append_process(&self, process: &Process) -> TreeIter {
        let (read_bytes, write_bytes, read_bytes_string, write_bytes_string, read_coef, write_coef) = match &process.io_stats {
            Ok(s) => (
                s.read_bytes,
                s.write_bytes,
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
            Err(_) => (0, 0, String::from("-"), String::from("-"), 0.0, 0.0),
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

        self.model.insert_with_values(
            None,
            &[
                Column::PID as u32, Column::Name as u32,
                Column::ReadBytes as u32, Column::WriteBytes as u32,
                Column::ReadBytesRaw as u32, Column::WriteBytesRaw as u32,
                Column::ReadBytesColor as u32, Column::WriteBytesColor as u32,
            ],
            &[&process.pid, &name, &read_bytes_string, &write_bytes_string, &read_bytes, &write_bytes, &bgr, &bgw],
            )
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

pub fn send_signal(pid: i32, signal: nix::sys::signal::Signal) -> nix::Result<()> {
    let pid = nix::unistd::Pid::from_raw(pid);
    nix::sys::signal::kill(pid, signal)
}

fn send_signal_to_selected(tree: &TreeView, signal: nix::sys::signal::Signal) -> nix::Result<()> {
    let selection = tree.get_selection();
    if let Some((model, iter)) = selection.get_selected() {
        let pid = model.get_value(&iter, Column::PID as i32);
        if let Some(pid) = pid.get::<i32>() {
            return send_signal(pid, signal);
        }
    }
    Ok(())
}
