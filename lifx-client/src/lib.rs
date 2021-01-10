mod any_message;
pub mod transport;

pub use any_message::AnyMessage;
pub use transport::DeviceAddress;

/*

Should abstract away:
* sending the source ID
* sequence numbers
* waiting for an acknowledgement/response

*/
