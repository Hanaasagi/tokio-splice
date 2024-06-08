use std::future::poll_fn;
use std::io::{Error, ErrorKind, Result};
use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use libc;
use tokio::io::{AsyncRead, AsyncWrite, Interest};

/// the size of PIPE_BUF
const PIPE_SIZE: usize = 65536;

/// splice()  moves  data between two file descriptors without copying between kernel address space and user address space.
/// It transfers up to len bytes of data from the file descriptor fd_in to the file descriptor fd_out,
/// where one of the  file  descriptors must refer to a pipe.
/// The following semantics apply for fd_in and off_in:
/// •  If fd_in refers to a pipe, then off_in must be NULL.
/// •  If  fd_in does not refer to a pipe and off_in is NULL, then bytes are read from fd_in starting from the file offset, and
///    the file offset is adjusted appropriately.
/// •  If fd_in does not refer to a pipe and off_in is not NULL, then off_in must point to a buffer which specifies the  start‐
///    ing offset from which bytes will be read from fd_in; in this case, the file offset of fd_in is not changed.
/// The flags argument is a bit mask that is composed by ORing together zero or more of the following values:
/// SPLICE_F_MOVE
///        Attempt  to move pages instead of copying.  This is only a hint to the kernel: pages may still be copied if the ker‐
///        nel cannot move the pages from the pipe, or if the pipe buffers don't refer to full pages.  The initial  implementa‐
///        tion  of this flag was buggy: therefore starting in Linux 2.6.21 it is a no-op (but is still permitted in a splice()
///        call); in the future, a correct implementation may be restored.
/// SPLICE_F_NONBLOCK
///        Do not block on I/O.  This makes the splice pipe operations nonblocking, but splice() may nevertheless block because
///        the file descriptors that are spliced to/from may block (unless they have the O_NONBLOCK flag set).
/// SPLICE_F_MORE
///        More  data  will  be  coming in a subsequent splice.  This is a helpful hint when the fd_out refers to a socket (see
///        also the description of MSG_MORE in send(2), and the description of TCP_CORK in tcp(7)).
/// SPLICE_F_GIFT
///        Unused for splice(); see vmsplice(2).
#[inline]
fn splice(fd_in: RawFd, fd_out: RawFd, size: usize) -> isize {
    unsafe {
        libc::splice(
            fd_in,
            std::ptr::null_mut::<libc::loff_t>(),
            fd_out,
            std::ptr::null_mut::<libc::loff_t>(),
            size,
            // NOTE: https://stackoverflow.com/questions/7463689/most-efficient-way-to-copy-a-file-in-linux/7464280#7464280
            // Both the man page for splice and the comments in the kernel source say that the SPLICE_F_MOVE flag should provide this functionality.
            // Unfortunately, the support for SPLICE_F_MOVE was yanked in 2.6.21 (back in 2007) and never replaced.
            // If you search the kernel sources, you will find SPLICE_F_MOVE is not actually referenced anywhere.
            // The bottom line is that splice from one file to another calls memcpy to move the data; it is not zero-copy.
            // libc::SPLICE_F_MORE | libc::SPLICE_F_NONBLOCK, // | libc::SPLICE_F_MOVE,
            libc::SPLICE_F_NONBLOCK,
        )
    }
}

/// Linux Pipe
#[repr(C)]
struct Pipe(RawFd, RawFd);

impl Pipe {
    /// Create a pipe
    fn new() -> Result<Self> {
        let mut pipe = std::mem::MaybeUninit::<[libc::c_int; 2]>::uninit();
        unsafe {
            // pipe() creates a pipe, a unidirectional data channel that can be used for interprocess communication.
            // The array pipefd is used to return two file descriptors referring to the ends of the pipe.
            // pipefd[0] refers to the read end of the pipe.
            // pipefd[1] refers to the write end of the pipe.
            // Data written to the write end of the pipe is buffered by the kernel until
            // it is read from the read end of the pipe.  For further details, see pipe(7).
            //
            // O_DIRECT (since Linux 3.4)
            //    Create a pipe that performs I/O in "packet" mode.  Each write(2) to the pipe is dealt with as a separate packet, and
            //    read(2)s from the pipe will read one packet at a time.  Note the following points:
            //    •  Writes  of  greater than PIPE_BUF bytes (see pipe(7)) will be split into multiple packets.  The constant PIPE_BUF
            //       is defined in <limits.h>.
            //    •  If a read(2) specifies a buffer size that is smaller than the next packet, then the requested number of bytes are
            //       read,  and the excess bytes in the packet are discarded.  Specifying a buffer size of PIPE_BUF will be sufficient
            //       to read the largest possible packets (see the previous point).
            //    •  Zero-length packets are not supported.  (A read(2) that specifies a buffer size of zero is a no-op,  and  returns
            //         0.)
            if libc::pipe2(
                pipe.as_mut_ptr() as *mut libc::c_int,
                // libc::O_DIRECT |libc::O_CLOEXEC | libc::O_NONBLOCK,
                libc::O_CLOEXEC | libc::O_NONBLOCK,
            ) < 0
            {
                return Err(Error::last_os_error());
            }
            let [r_fd, w_fd] = pipe.assume_init();
            libc::fcntl(w_fd, libc::F_SETPIPE_SZ, PIPE_SIZE);
            Ok(Pipe(r_fd, w_fd))
        }
    }

    #[inline]
    fn read_fd(&self) -> RawFd {
        self.0
    }

    #[inline]
    fn write_fd(&self) -> RawFd {
        self.1
    }
}

impl Drop for Pipe {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.0);
            libc::close(self.1);
        }
    }
}

struct CopyBuffer<R, W> {
    read_done: bool,
    need_flush: bool,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Pipe,
    //
    _marker_r: PhantomData<R>,
    _marker_w: PhantomData<W>,
}

impl<R, W> CopyBuffer<R, W>
where
    R: Stream + Unpin,
    W: Stream + Unpin,
{
    fn new(buf: Pipe) -> Self {
        Self {
            read_done: false,
            need_flush: false,
            pos: 0,
            cap: 0,
            amt: 0,
            buf,
            _marker_r: PhantomData,
            _marker_w: PhantomData,
        }
    }

    fn poll_fill_buf(&mut self, cx: &mut Context<'_>, stream: &mut R) -> Poll<Result<usize>> {
        loop {
            ready!(stream.poll_read_ready_n(cx))?;

            let res = stream.try_io_n(Interest::READABLE, || {
                match splice(stream.as_raw_fd(), self.buf.write_fd(), isize::MAX as usize) {
                    size if size >= 0 => Ok(size as usize),
                    _ => {
                        let err = Error::last_os_error();
                        match err.raw_os_error() {
                            Some(e) if e == libc::EWOULDBLOCK || e == libc::EAGAIN => {
                                Err(ErrorKind::WouldBlock.into())
                            }
                            _ => Err(err),
                        }
                    }
                }
            });

            match res {
                Ok(size) => {
                    if self.cap == size {
                        self.read_done = true;
                    }
                    self.cap = size;
                    return Poll::Ready(res);
                }
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        continue;
                        // return Poll::Pending;
                    }

                    return Poll::Ready(Err(e));
                }
            }
        }
    }

    fn poll_write_buf(&mut self, cx: &mut Context<'_>, stream: &mut W) -> Poll<Result<usize>> {
        loop {
            ready!(stream.poll_write_ready_n(cx)?);

            let res = stream.try_io_n(Interest::WRITABLE, || {
                match splice(self.buf.read_fd(), stream.as_raw_fd(), self.cap - self.pos) {
                    size if size >= 0 => Ok(size as usize),
                    _ => {
                        let err = Error::last_os_error();
                        match err.raw_os_error() {
                            Some(e) if e == libc::EWOULDBLOCK || e == libc::EAGAIN => {
                                Err(ErrorKind::WouldBlock.into())
                            }
                            _ => Err(err),
                        }
                    }
                }
            });

            match res {
                Ok(_) => return Poll::Ready(res),
                Err(e) => {
                    if e.kind() == ErrorKind::WouldBlock {
                        continue;
                    }

                    return Poll::Ready(Err(e));
                }
            }
        }
    }

    fn poll_flush_buf(&mut self, cx: &mut Context<'_>, stream: &mut W) -> Poll<Result<()>> {
        Pin::new(stream).poll_flush(cx)
    }
}

impl<R, W> CopyBuffer<R, W>
where
    R: Stream + Unpin,
    W: Stream + Unpin,
{
    fn poll_copy(&mut self, cx: &mut Context<'_>, r: &mut R, w: &mut W) -> Poll<Result<u64>> {
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            if self.pos == self.cap && !self.read_done {
                self.pos = 0;
                self.cap = 0;

                match self.poll_fill_buf(cx, r) {
                    Poll::Ready(Ok(_)) => (),
                    Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    Poll::Pending => {
                        // Try flushing when the reader has no progress to avoid deadlock
                        // when the reader depends on buffered writer.
                        if self.need_flush {
                            ready!(self.poll_flush_buf(cx, w))?;
                            self.need_flush = false;
                        }

                        return Poll::Pending;
                    }
                };
            }

            while self.pos < self.cap {
                let size = ready!(self.poll_write_buf(cx, w))?;

                if size == 0 {
                    return Poll::Ready(Err(Error::new(
                        ErrorKind::WriteZero,
                        "write zero byte into writer",
                    )));
                } else {
                    self.pos += size;
                    self.amt += size as u64;
                    self.need_flush = true;
                }
            }

            // If pos larger than cap, this loop will never stop.
            // In particular, user's wrong poll_write implementation returning
            // incorrect written length may lead to thread blocking.
            debug_assert!(
                self.pos <= self.cap,
                "writer returned length larger than input slice"
            );

            // If we've written all the data and we've seen EOF, flush out the
            // data and finish the transfer.
            if self.pos == self.cap && self.read_done {
                ready!(self.poll_flush_buf(cx, w))?;
                return Poll::Ready(Ok(self.amt));
            }
        }
    }
}

enum TransferState<SR, SW> {
    Running(CopyBuffer<SR, SW>),
    ShuttingDown(u64),
    Done(u64),
}

fn transfer_one_direction<SL, SR>(
    cx: &mut Context<'_>,
    state: &mut TransferState<SL, SR>,
    r: &mut SL,
    w: &mut SR,
) -> Poll<Result<u64>>
where
    SL: Stream + Unpin,
    SR: Stream + Unpin,
{
    loop {
        match state {
            TransferState::Running(buf) => {
                let count = ready!(buf.poll_copy(cx, r, w))?;

                *state = TransferState::ShuttingDown(count);
            }
            TransferState::ShuttingDown(count) => {
                ready!(Pin::new(&mut *w).poll_shutdown(cx))?;

                *state = TransferState::Done(*count);
            }
            TransferState::Done(count) => return Poll::Ready(Ok(*count)),
        }
    }
}

/// `AsyncRead` + `AsyncWrite` + `AsRawFd` Wrapper for stream.
/// This trait is auto implemented for `TcpStream` and `UnixStream`.
pub trait Stream: AsyncRead + AsyncWrite + AsRawFd {
    fn poll_read_ready_n(&self, cx: &mut Context<'_>) -> Poll<Result<()>>;
    fn poll_write_ready_n(&self, cx: &mut Context<'_>) -> Poll<Result<()>>;
    fn try_io_n<R>(&self, interest: Interest, f: impl FnOnce() -> Result<R>) -> Result<R>;
}

/// Copies data in both directions between `a` and `b`.
///
/// This function returns a future that will read from both streams,
/// writing any data read to the opposing stream.
/// This happens in both directions concurrently.
pub async fn zero_copy_bidirectional<A, B>(a: &mut A, b: &mut B) -> Result<(u64, u64)>
where
    A: Stream + Unpin,
    B: Stream + Unpin,
{
    let mut a_to_b = TransferState::Running(CopyBuffer::new(Pipe::new()?));
    let mut b_to_a = TransferState::Running(CopyBuffer::new(Pipe::new()?));

    poll_fn(|cx| {
        let a_to_b = transfer_one_direction(cx, &mut a_to_b, a, b)?;
        let b_to_a = transfer_one_direction(cx, &mut b_to_a, b, a)?;

        let a_to_b = ready!(a_to_b);
        let b_to_a = ready!(b_to_a);

        Poll::Ready(Ok((a_to_b, b_to_a)))
    })
    .await
}

mod tests {}

use tokio::net::{TcpStream, UnixStream};
macro_rules! impl_stream_for {
    ($stream: ident) => {
        impl Stream for $stream {
            #[inline]
            fn poll_read_ready_n(&self, cx: &mut Context<'_>) -> Poll<Result<()>> {
                self.poll_read_ready(cx)
            }
            #[inline]
            fn poll_write_ready_n(&self, cx: &mut Context<'_>) -> Poll<Result<()>> {
                self.poll_write_ready(cx)
            }
            #[inline]
            fn try_io_n<R>(&self, interest: Interest, f: impl FnOnce() -> Result<R>) -> Result<R> {
                self.try_io(interest, f)
            }
        }
    };
}
impl_stream_for!(TcpStream);
impl_stream_for!(UnixStream);
