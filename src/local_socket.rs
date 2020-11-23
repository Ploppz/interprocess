//! Local sockets, an IPC primitive featuring a server and multiple clients connecting to that server using a filesystem path inside a special namespace, each having a private connection to that server.
//!
//! Local sockets are not a real IPC method implemented by the OS — they were introduced because of the difference between named pipes on Windows and Unix: named pipes on Windows are almost the same as Unix domain sockets on Linux while Unix named pipes (which are referred to as FIFO files in this crate to avoid confusion) are like unnamed pipes but identifiable with a filesystem path: there's no distinction between writers and the first reader takes all. **Simply put, local sockets use named pipes on Windows and Unix domain sockets on Unix.**
//!
//! ## Platform-specific namespaces
//! There's one more problem regarding platform differences: since only Linux supports putting Ud-sockets in a separate namespace which is isolated from the filesystem, the `LocalSocketName`/`LocalSocketNameBuf` types are used to identify local sockets rather than `OsStr`/`OsString`: on Unix platforms other than Linux, which includes macOS, all flavors of BSD and possibly other Unix-like systems, the only way to name a Ud-socket is to use a filesystem path. As such, those platforms don't have the namespaced socket creation method available. Complicatng matters further, Windows does not support named pipes in the normal filesystem, meaning that namespaced local sockets are the only functional method on Windows. As a way to solve this issue, `LocalSocketName`/`LocalSocketNameBuf` only provide creation in a platform-specific way, meaning that crate users are required to use conditional compilation to decide on the name for the socket names.

use std::{
    io::{self, IoSlice, IoSliceMut, prelude::*},
    fmt::{self, Formatter, Debug},
    iter::FusedIterator,
    borrow::Cow,
    ffi::{OsStr, OsString, CStr, CString},
    path::{Path, PathBuf},
    str,
};

impmod!{local_socket,
    name_type_support_query as name_type_support_query_impl,
    NAME_TYPE_ALWAYS_SUPPORTED as NAME_TYPE_ALWAYS_SUPPORTED_REAL,
    to_local_socket_name_osstr,
    to_local_socket_name_osstring,
    LocalSocketListener as LocalSocketListenerImpl,
    LocalSocketStream as LocalSocketStreamImpl,
}

/// A local socket server, listening for connections.
///
/// # Example
/// ```no_run
/// use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
/// use std::io::{self, prelude::*, BufReader};
///
/// fn handle_error(conn: io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
///     match conn {
///         Ok(val) => Some(val),
///         Err(error) => {
///             eprintln!("Incoming connection failed: {}", error);
///             None
///         }
///     }
/// }
///
/// let listener = LocalSocketListener::bind("/tmp/example.sock")?;
/// for mut conn in listener.incoming().filter_map(handle_error) {
///     conn.write_all(b"Hello from server!\n")?;
///     let mut conn = BufReader::new(conn);
///     let mut buffer = String::new();
///     conn.read_line(&mut buffer);
///     println!("Client answered: {}", buffer);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct LocalSocketListener {
    inner: LocalSocketListenerImpl,
}
impl LocalSocketListener {
    /// Creates a socket server with the specified local socket name.
    #[inline(always)]
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        Ok(Self {inner: LocalSocketListenerImpl::bind(name)?})
    }
    /// Listens for incoming connections to the socket, blocking until a client is connected.
    ///
    /// See [`incoming`] for a convenient way to create a main loop for a server.
    ///
    /// [`incoming`]: #method.incoming " "
    #[inline(always)]
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        Ok(LocalSocketStream {inner: self.inner.accept()?})
    }
    /// Creates an infinite iterator which calls `accept()` with each iteration. Used together with `for` loops to conveniently create a main loop for a socket server.
    ///
    /// # Example
    /// See the struct-level documentation for a full example which already uses this method.
    #[inline(always)]
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming::from(self)
    }
}
impl Debug for LocalSocketListener {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

/// An infinite iterator over incoming client connections of a [`LocalSocketListener`].
///
/// This iterator is created by the [`incoming`] method on [`LocalSocketListener`] — see its documentation for more.
///
/// [`LocalSocketListener`]: struct.LocalSocketListener.html " "
/// [`incoming`]: struct.LocalSocketListener.html#method.incoming " "
#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a LocalSocketListener,
}
impl<'a> From<&'a LocalSocketListener> for Incoming<'a> {
    #[inline(always)]
    fn from(listener: &'a LocalSocketListener) -> Self {
        Self {listener}
    }
}
impl Iterator for Incoming<'_> {
    type Item = io::Result<LocalSocketStream>;
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.accept())
    }
    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}
impl FusedIterator for Incoming<'_> {}

/// A local socket byte stream, obtained eiter from [`LocalSocketListener`] or by connecting to an existing local socket.
///
/// # Example
/// ```no_run
/// use interprocess::local_socket::LocalSocketStream;
/// use std::io::{prelude::*, BufReader};
///
/// // Replace the path as necessary on Windows.
/// let mut conn = LocalSocketStream::connect("/tmp/example.sock")?;
/// conn.write_all(b"Hello from client!\n")?;
/// let mut conn = BufReader::new(conn);
/// let mut buffer = String::new();
/// conn.read_line(&mut buffer)?;
/// println!("Server answered: {}", buffer);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// [`LocalSocketListener`]: struct.LocalSocketListener.html " "
pub struct LocalSocketStream {
    inner: LocalSocketStreamImpl,
}
impl LocalSocketStream {
    /// Connects to a remote local socket server.
    #[inline(always)]
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        Ok(Self {inner: LocalSocketStreamImpl::connect(name)?})
    }
}
impl Read for LocalSocketStream {
    #[inline(always)]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
    #[inline(always)]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }
}
impl Write for LocalSocketStream {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    #[inline(always)]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }
    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
impl Debug for LocalSocketStream {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}
impl_handle_manip!(LocalSocketStream);

/// A name for a local socket.
///
/// Due to vast differences between platforms in terms of how local sockets are named, there needs to be a way to store and process those in a unified way while also retaining platform-specific pecularities. `LocalSocketName` aims to bridge the gap between portability and platform-specific correctness.
///
/// # Creation
/// A separate trait is used to create names from basic strings: [`ToLocalSocketName`]. Aside from being conveniently implemented on every single string type in the standard library, it also provides some special processing. Please read its documentation if you haven't already — the rest of this page assumes you did.
///
/// # Validity
/// As mentioned in the [module-level documentation], not all platforms support all types of local socket names. A name pointing to a filesystem location is only supported on Unix-like systems, and names pointing to an abstract namespace reserved specifically for local sockets are only available on Linux and Windows. Due to the diversity of those differences, `LocalSocketName` does not provide any forced validation by itself — the [`is_supported`] and [`is_always_supported`] checks are not enforced to succeed. Instead, they are intended as helpers for the process of user input validation, if any local socket names are ever read from environment variables, configuration files or other methods of user input.
///
/// If an invalid local socket name is used to create a local socket or connect to it, the creation/connection method will fail.
///
/// [`to_local_socket_name`]: trait.ToLocalSocketName.html " "
/// [module-level documentation]: index.html " "
/// [`is_supported`]: #method.is_supported " "
/// [`is_always_supported`]: #method.is_always_supported " "
pub struct LocalSocketName<'a> {
    inner: Cow<'a, OsStr>,
    namespaced: bool,
}
impl<'a> LocalSocketName<'a> {
    /// Returns `true` if the type of the name is supported by the OS, `false` otherwise.
    ///
    /// The check is performed at runtime. For a conservative compile-time check, see [`is_always_supported`].
    ///
    /// [`is_always_supported`]: #method.is_always_supported " "
    #[inline]
    pub fn is_supported(&self) -> bool {
           (NameTypeSupport::query().namespace_supported() && self.is_namespaced())
        || (NameTypeSupport::query().paths_supported() && !self.is_path())
    }
    /// Returns `true` if the type of the name is supported by the OS, `false` otherwise.
    ///
    /// The check is performed at compile-time. For a check which might return a more permissive result on certain platforms by checking for support at runtime, see [`is_supported`].
    ///
    /// [`is_supported`]: #method.is_supported " "
    #[inline]
    pub fn is_always_supported(&self) -> bool {
           (NameTypeSupport::ALWAYS_AVAILABLE.namespace_supported() && self.is_namespaced())
        || (NameTypeSupport::ALWAYS_AVAILABLE.paths_supported() && self.is_path())
    }
    /// Returns `true` if the value is a namespaced name, `false` otherwise.
    #[inline(always)]
    pub const fn is_namespaced(&self) -> bool {
        self.namespaced
    }
    /// Returns `true` if the value is a filesystem path, `false` otherwise.
    #[inline(always)]
    pub const fn is_path(&self) -> bool {
        !self.namespaced
    }
    /// Returns the name as an `OsStr`. The returned value does not retain the type of the name (whether it was a filesystem path or a namespaced name).
    ///
    /// If you need the value as an owned `OsString` instead, see [`into_inner`].
    ///
    /// [`into_inner`]: #method.into_inner " "
    #[inline(always)]
    pub fn inner(&'a self) -> &'a OsStr {
        &self.inner
    }
    /// Returns the name as an `OsString`. The returned value does not retain the type of the name (whether it was a filesystem path or a namespaced name).
    ///
    /// If you need the value as a borrowed `OsStr` instead, see [`inner`].
    ///
    /// [`inner`]: #method.inner " "
    #[inline(always)]
    pub fn into_inner(self) -> OsString {
        self.inner.into_owned()
    }
    /// Returns the name as a *borrowed* `Cow<'_, OsStr>`. The returned value does not retain the type of the name (whether it was a filesystem path or a namespaced name).
    ///
    /// If you need the value as a borrowed `OsStr`, see [`inner`]; if you need the value as an owned `OsString`, see [`into_inner`].  If you need to take ownership of the `Cow`, see `into_inner_cow`.
    ///
    /// [`inner`]: #method.inner " "
    /// [`into_inner`]: #method.into_inner " "
    /// [`into_inner_cow`]: #method.into_inner_cow " "
    #[inline(always)]
    pub const fn inner_cow(&'a self) -> &'a Cow<'a, OsStr> {
        &self.inner
    }
    /// Returns the name as a `Cow<'_, OsStr>`. The returned value does not retain the type of the name (whether it was a filesystem path or a namespaced name).
    ///
    /// If you need the value as a borrowed `OsStr`, see [`inner`]; if you need the value as an owned `OsString`, see [`into_inner`]. If you don't need to take ownership of the `Cow`, see `inner_cow`.
    ///
    /// [`inner`]: #method.inner " "
    /// [`into_inner`]: #method.into_inner " "
    /// [`inner_cow`]: #method.inner_cow " "
    #[inline(always)]
    pub fn into_inner_cow(self) -> Cow<'a, OsStr> {
        self.inner
    }
    #[inline(always)]
    pub(crate) const fn from_raw_parts(inner: Cow<'a, OsStr>, namespaced: bool) -> Self {
        Self {inner, namespaced}
    }
}

/// Represents which kinds of identifiers can be used for a local socket's name on the current platform.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NameTypeSupport {
    /// Only filesystem paths can be used.
    ///
    /// This is true for all Unix/POSIX and Unix-like systems other than Linux.
    OnlyPaths,
    /// Only names in an dedicated namespace can be used.
    ///
    /// This is true only for Windows.
    OnlyNamespaced,
    /// Both of the above options are available.
    ///
    /// This is true only for Linux.
    Both,
}
impl NameTypeSupport {
    /// The types of local socket names supported on the current platform regardless of the environment and OS version.
    ///
    /// On most platforms, the value is known at compile time, i.e. the support for paths wasn't introduced in a specific version of the OS or isn't known to be supported at all. **Currently, this includes all supported OSes.** If support is added for an OS which added this functionality in a specific version, this constant will be the most restrictive value for that platform, with [`query`] possibly returning the actual value according to the current version of the OS.
    ///
    /// Simply put, you should probably just use this value for consistency across platforms, unless you really need a specific name type to be supported.
    ///
    /// [`query`]: #method.query " "
    pub const ALWAYS_AVAILABLE: Self = NAME_TYPE_ALWAYS_SUPPORTED_REAL;
    /// Returns the types of local socket names supported on the current platform with the current environment.
    ///
    /// On most platforms, the value is known at compile time, i.e. the support for one of the types wasn't introduced in an update to the OS or isn't known to be supported at all. **Currently, this includes all supported OSes.** For compatibility with OSes which might add the functionality in the future starting with a specific version, this function isn't a `const fn` — see [`ALWAYS_AVAILABLE`] if you need a constant expression.
    ///
    /// [`ALWAYS_AVAILABLE`]: #associatedconstant.ALWAYS_AVAILABLE " "
    #[inline(always)]
    pub fn query() -> Self {
        name_type_support_query_impl()
    }

    /// Returns `true` if, according to `self`, filesystem-based local sockets are supported; `false` otherwise.
    #[inline(always)]
    pub const fn paths_supported(self) -> bool {
        matches!(self, Self::OnlyPaths | Self::Both)
    }
    /// Returns `true` if, according to `self`, namespaced local socket names are supported; `false` otherwise.
    #[inline(always)]
    pub const fn namespace_supported(self) -> bool {
        matches!(self, Self::OnlyNamespaced | Self::Both)
    }
}
/// Types which can be converted to a local socket name.
///
/// The difference between this trait and [`TryInto`]`<`[`LocalSocketName`]`>` is that the latter does not constrain the error type to be [`io::Error`] and thus is not compatible with many types from the standard library which are widely expected to be convertible to Unix domain socket paths. Additionally, this makes the special syntax for namespaced sockets possible (see below).
///
/// ## `@` syntax for namespaced paths
/// As mentioned in the [`LocalSocketName` documentation][`LocalSocketName`], there are two types of which local socket names can be: filesystem paths and namespaced names. Those are isolated from each other — there's no portable way to represent one using another, though certain OSes might provide ways to do so — Windows does, for example. To be able to represent both in a platform-independent fashion, a special syntax was implemented in implementations of this trait on types from the standard library: "@ syntax".
///
/// The feature, in its core, is extremely simple: if the first character in a string is the @ character, the value of the string is interpreted and stored as a namespaced name (otherwise, it's treated as a filesystem path); the @ character is then removed from the string (by taking a subslice which dosen't include it if a string slice is being used; for owned strings, it's simply removed from the string by shifting the entire string towards the beginning). **[`Path`] and [`PathBuf`] are not affected at all — those have explicit path semantics and therefore cannot logically represent namespaced names.**
///
/// This feature is extremely useful both when using hardcoded literals and accepting user input for the path, but sometimes you might want to prevent this behavior. In such a case, you have the following possible approaches:
/// - If the string is a [`OsStr`]/[`OsString`], it can be cheaply converted to a [`Path`]/[`PathBuf`], which do not support the @ syntax
/// - If the string is a [`str`]/[`String`], it can be cheaply converted to [`OsStr`]/[`OsString`]; then the above method can be applied
/// - If the string is a [`CStr`]/[`CString`], it can be converted to [`str`]/[`String`] using the following code:
/// ```
/// # use std::{
/// #     str::Utf8Error,
/// #     ffi::{CStr, CString},
/// # };
/// fn cstr_to_str(val: &CStr) -> Result<&str, Utf8Error> {
///     std::str::from_utf8(val.to_bytes_with_nul())
/// }
/// fn cstring_to_string(val: CString) -> String {
///     String::from_utf8_lossy(&val.into_bytes_with_nul()).into()
/// }
/// ```
/// Then, the method for [`str`]/[`String`] can be applied.
///
/// None of the above conversions perform memory allocations — the only expensive one is [`CStr`]/[`CString`] which performs a check for valid UTF-8.
///
/// [`LocalSocketName`]: struct.LocalSocketName.html " "
/// [`TryInto`]: https://doc.rust-lang.org/std/convert/trait.TryInto.html " "
/// [`str`]: https://doc.rust-lang.org/std/primitive.str.html " "
/// [`String`]: https://doc.rust-lang.org/std/string/struct.String.html " "
/// [`OsStr`]: https://doc.rust-lang.org/std/ffi/struct.OsStr.html " "
/// [`OsString`]: https://doc.rust-lang.org/std/ffi/struct.OsString.html " "
/// [`CStr`]: https://doc.rust-lang.org/std/ffi/struct.CStr.html " "
/// [`CString`]: https://doc.rust-lang.org/std/ffi/struct.CString.html " "
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html " "
/// [`PathBuf`]: https://doc.rust-lang.org/std/path/struct.PathBuf.html " "
pub trait ToLocalSocketName<'a> {
    /// Performs the conversion to a local socket name.
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>>;
}
// TODO document inpls for symmetry with ud-sockets
impl<'a> ToLocalSocketName<'a> for &'a Path {
    #[inline(always)]
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>> {
        Ok(
            LocalSocketName::from_raw_parts(
                Cow::Borrowed(self.as_os_str()),
                false,
            )
        )
    }
}
impl ToLocalSocketName<'static> for PathBuf {
    #[inline(always)]
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'static>> {
        Ok(
            LocalSocketName::from_raw_parts(
                Cow::Owned(self.into_os_string()),
                false,
            )
        )
    }
}
impl<'a> ToLocalSocketName<'a> for &'a OsStr {
    #[inline(always)]
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>> {
        Ok(to_local_socket_name_osstr(self))
    }
}
impl ToLocalSocketName<'static> for OsString {
    #[inline(always)]
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'static>> {
        Ok(to_local_socket_name_osstring(self))
    }
}
impl<'a> ToLocalSocketName<'a> for &'a str {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>> {
        OsStr::new(self).to_local_socket_name()
    }
}
impl ToLocalSocketName<'static> for String {
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'static>> {
        // OsString docs misleadingly state that a conversion from String requires reallocating
        // and copying, but, according to the std sources, that is not true on any platforms.
        OsString::from(self).to_local_socket_name()
    }
}
impl<'a> ToLocalSocketName<'a> for &'a CStr {
    #[inline]
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'a>> {
        str::from_utf8(self.to_bytes_with_nul())
            .map(|x| to_local_socket_name_osstr(OsStr::new(x)))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}
impl ToLocalSocketName<'static> for CString {
    #[inline]
    fn to_local_socket_name(self) -> io::Result<LocalSocketName<'static>> {
        String::from_utf8(self.into_bytes_with_nul())
            .map(|x| to_local_socket_name_osstring(OsString::from(x)))
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }
}

#[cfg(test)]
mod test {
    use crate::local_socket::{LocalSocketListener, LocalSocketStream};
    use crate::rand_string;

    #[test]
    fn basic() {
        use std::{
            io::{self, prelude::*, BufReader},
            sync::{Arc, Barrier},
        };

        let socket_name = format!("/tmp/{}.sock", rand_string(10));

        // We're using a barrier here to avoid the client attempting to connect
        // before the server creates the socket
        let barrier = Arc::new(Barrier::new(2));
        let server_barrier = Arc::clone(&barrier);
 
        let socket_name2 = socket_name.clone();       
        std::thread::spawn(move || {
            fn handle_error(
                connection: io::Result<LocalSocketStream>,
            ) -> Option<LocalSocketStream> {
                match connection {
                    Ok(val) => Some(val),
                    Err(error) => {
                        panic!("Incoming connection failed: {}", error);
                    }
                }
            }

            let listener = LocalSocketListener::bind(socket_name2).unwrap();
            server_barrier.wait();
            for mut conn in listener.incoming().filter_map(handle_error) {
                println!("Incoming connection!");
                conn.write_all(b"Hello from server!\n").unwrap();
                // Add buffering to the connection to read a line.
                let mut conn = BufReader::new(conn);
                let mut buffer = String::new();
                conn.read_line(&mut buffer).unwrap();
                println!("Client answered: {}", buffer);
            }
        });

        barrier.wait();
        let h2 = std::thread::spawn(move || {
            let mut conn = LocalSocketStream::connect(socket_name).unwrap();
            conn.write_all(b"Hello from client!\n").unwrap();
            let mut conn = BufReader::new(conn);
            let mut buffer = String::new();
            conn.read_line(&mut buffer).unwrap();
            println!("Server answered: {}", buffer);
        });
        h2.join().unwrap();
    }
     #[test]
    fn high_payload() {
        use std::{
            io::{self, prelude::*, BufReader},
            sync::{Arc, Barrier},
        };

        let socket_name = format!("/tmp/{}.sock", rand_string(10));

        // We're using a barrier here to avoid the client attempting to connect
        // before the server creates the socket
        let barrier = Arc::new(Barrier::new(2));
        let server_barrier = Arc::clone(&barrier);

        let socket_name2 = socket_name.clone();
        std::thread::spawn(move || {
            fn handle_error(
                connection: io::Result<LocalSocketStream>,
            ) -> Option<LocalSocketStream> {
                match connection {
                    Ok(val) => Some(val),
                    Err(error) => {
                        panic!("Incoming connection failed: {}", error);
                    }
                }
            }

            let listener = LocalSocketListener::bind(socket_name2).unwrap();
            server_barrier.wait();
            for mut conn in listener.incoming().filter_map(handle_error) {
                println!("Incoming connection!");
                conn.write_all(b"Hello from server!\n").unwrap();
                // Add buffering to the connection to read a line.
                let mut conn = BufReader::new(conn);
                let mut buffer: Vec<u8> = vec![0; 10_000_000];
                conn.read_exact(&mut buffer).unwrap();
                println!("Client answered: (len={})", buffer.len());
            }
        });

        barrier.wait();
        let h2 = std::thread::spawn(move || {
            let mut conn = LocalSocketStream::connect(socket_name).unwrap();
            const LEN: usize = 10_000_000;
            let data = (0..LEN).map(|i| (i%256) as u8).collect::<Vec<u8>>();
            conn.write_all(&data).unwrap();
            let mut conn = BufReader::new(conn);
            let mut buffer = String::new();
            conn.read_line(&mut buffer).unwrap();
            println!("Server answered: {}", buffer);
        });
        h2.join().unwrap();
    }
}
