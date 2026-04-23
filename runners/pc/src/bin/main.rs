use solo_pc::{mount_filesystems, Board, UserInterface};
use trussed::service::SeedableRng;

fn main() {
    let store = mount_filesystems();
    let rng = chacha20::ChaCha8Rng::from_seed([0u8; 32]);
    let board = Board::new(rng, store, UserInterface::default());
    let mut _trussed = trussed::service::Service::new(board);
    println!("hello trussed");
}
