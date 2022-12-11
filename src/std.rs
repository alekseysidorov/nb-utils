use std::io::ErrorKind;

/// Converts [`std::io::Result`] into the [`nb::Result`].
/// 
/// This conversion relies on the [`std::io::ErrorKind::WouldBlock`] logic. 
/// So you should enable non-blocking logic on the std io primitives. 
/// For, example one should set the [`std::net::TcpStream::set_nonblocking`] to true.
pub trait IntoNbResult<T, E> {
    /// Performs the types conversion.
    fn into_nb_result(self) -> nb::Result<T, E>;
}

impl<T> IntoNbResult<T, std::io::Error> for std::io::Result<T> {
    fn into_nb_result(self) -> nb::Result<T, std::io::Error> {
        match self {
            Ok(value) => Ok(value),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Err(nb::Error::WouldBlock),
            Err(err) => Err(nb::Error::Other(err)),
        }
    }
}
