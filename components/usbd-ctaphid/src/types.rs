
use embedded_time::duration::Milliseconds;

// Status to indicate Whether or not to send keepalive messages
pub enum Status {
    // No need
    Idle,
    // Should schedule take with given period in miliseconds
    ReceivedData(Milliseconds),
}

pub enum KeepaliveStatus {
    Processing = 1,
    UpNeeded = 2,
}
