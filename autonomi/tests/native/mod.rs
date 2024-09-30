use autonomi;

#[cfg(feature = "files")]
mod file;
#[cfg(feature = "data")]
mod put;
#[cfg(feature = "registers")]
mod register;

pub type Client = autonomi::native::client::NativeClient;
