use std::{
    io::{self, IoSlice, IoSliceMut, prelude::*},
    fmt::{self, Formatter, Debug},
    ffi::{OsStr, OsString, CStr, CString},
    borrow::Cow,
    os::unix::{
        io::{AsRawFd, IntoRawFd, FromRawFd},
        ffi::{OsStrExt, OsStringExt},
    },
};
use crate::local_socket::{
    NameTypeSupport,
    LocalSocketName,
    ToLocalSocketName,
};
use super::udsocket::{
    UdStreamListener,
    UdSocketPath,
    UdStream,
};

pub(crate) struct LocalSocketListener {
    inner: UdStreamListener,
}
impl LocalSocketListener {
    #[inline]
    pub fn bind<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        let inner = UdStreamListener::bind(path)?;
        Ok(Self {inner})
    }
    #[inline(always)]
    pub fn accept(&self) -> io::Result<LocalSocketStream> {
        let inner = self.inner.accept()?;
        Ok(LocalSocketStream {inner})
    }
}
impl Debug for LocalSocketListener {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketListener")
            .field("file_descriptor", &self.inner.as_raw_fd())
            .finish()
    }
}
impl AsRawFd for LocalSocketListener {
    #[inline(always)]
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}
impl IntoRawFd for LocalSocketListener {
    #[inline(always)]
    fn into_raw_fd(self) -> i32 {
        self.inner.into_raw_fd()
    }
}
impl FromRawFd for LocalSocketListener {
    #[inline(always)]
    unsafe fn from_raw_fd(fd: i32) -> Self {
        Self {inner: UdStreamListener::from_raw_fd(fd)}
    }
}

pub(crate) struct LocalSocketStream {
    inner: UdStream,
}
impl LocalSocketStream {
    #[inline]
    pub fn connect<'a>(name: impl ToLocalSocketName<'a>) -> io::Result<Self> {
        let path = local_socket_name_to_ud_socket_path(name.to_local_socket_name()?)?;
        let inner = UdStream::connect(path)?;
        Ok(Self {inner})
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
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalSocketStream")
            .field("file_descriptor", &self.inner.as_raw_fd())
            .finish()
    }
}
impl AsRawFd for LocalSocketStream {
    #[inline(always)]
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}
impl IntoRawFd for LocalSocketStream {
    #[inline(always)]
    fn into_raw_fd(self) -> i32 {
        self.inner.into_raw_fd()
    }
}
impl FromRawFd for LocalSocketStream {
    #[inline(always)]
    unsafe fn from_raw_fd(fd: i32) -> Self {
        Self {inner: UdStream::from_raw_fd(fd)}
    }
}

#[inline]
fn local_socket_name_to_ud_socket_path(name: LocalSocketName<'_>) -> io::Result<UdSocketPath<'_>> {
    #[inline]
    fn cow_osstr_to_cstr(osstr: Cow<'_, OsStr>) -> io::Result<Cow<'_, CStr>> {
        match osstr {
            Cow::Borrowed(val) => {
                if val.as_bytes().last() == Some(&0) {
                    Ok(Cow::Borrowed(
                        CStr::from_bytes_with_nul(val.as_bytes())
                            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?
                    ))
                } else {
                    let owned = val.to_os_string();
                    Ok(Cow::Owned(
                        CString::new(owned.into_vec())?
                    ))
                }
            },
            Cow::Owned(val) => {
                Ok(Cow::Owned(
                    CString::new(val.into_vec())?
                ))
            },
        }
    }
    #[cfg(target_os = "linux")]
    if name.is_namespaced() {
        return Ok(UdSocketPath::Namespaced(
            cow_osstr_to_cstr(name.into_inner_cow())?
        ));
    }
    Ok(UdSocketPath::File(
        cow_osstr_to_cstr(name.into_inner_cow())?
    ))
}

#[inline(always)]
pub fn name_type_support_query() -> NameTypeSupport {
    NAME_TYPE_ALWAYS_SUPPORTED
}
#[cfg(target_os = "linux")]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::Both;
#[cfg(not(target_os = "linux"))]
pub const NAME_TYPE_ALWAYS_SUPPORTED: NameTypeSupport = NameTypeSupport::OnlyPaths;

const AT_SIGN: u8 = 0x40;

#[inline]
pub fn to_local_socket_name_osstr(mut val: &OsStr) -> LocalSocketName<'_> {
    let mut namespaced = false;
    if let Some(AT_SIGN) = val.as_bytes().get(0).copied() {
        if val.len() >= 2 {
            val = OsStr::from_bytes(&val.as_bytes()[1..]);
        } else {
            val = OsStr::from_bytes(&[]);
        }
        namespaced = true;
    }
    LocalSocketName::from_raw_parts(Cow::Borrowed(val), namespaced)
}
#[inline]
pub fn to_local_socket_name_osstring(mut val: OsString) -> LocalSocketName<'static> {
    let mut namespaced = false;
    if let Some(AT_SIGN) = val.as_bytes().get(0).copied() {
        let new_val = {
            let mut vec = val.into_vec();
            vec.remove(0);
            OsString::from_vec(vec)
        };
        val = new_val;
        namespaced = true;
    }
    LocalSocketName::from_raw_parts(Cow::Owned(val), namespaced)
}
