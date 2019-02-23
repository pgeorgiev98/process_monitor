mod interface;
use interface::Interface;

fn main() {
    let _interface = Interface::new().unwrap();

    Interface::exec();
}
