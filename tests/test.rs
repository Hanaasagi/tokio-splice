use std::io::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task;
use tokio_splice::zero_copy_bidirectional;

async fn echo_server(addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (mut socket, _) = listener.accept().await?;
        task::spawn(async move {
            let mut buf = vec![0; 1024];
            while let Ok(n) = socket.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                if socket.write_all(&buf[..n]).await.is_err() {
                    break;
                }
            }
        });
    }
}

async fn forwarding(mut stream1: TcpStream, forward_addr: &str) -> Result<()> {
    let mut stream2 = TcpStream::connect(forward_addr).await?;
    zero_copy_bidirectional(&mut stream1, &mut stream2).await?;
    Ok(())
}

async fn proxy_server(listen_addr: &str, forward_addr: &str) -> Result<()> {
    let listener = TcpListener::bind(listen_addr).await?;
    loop {
        let (conn, _) = listener.accept().await?;
        let forward_addr = forward_addr.to_string();
        task::spawn(async move { forwarding(conn, &forward_addr).await });
    }
}

#[tokio::test]
async fn test_proxy() -> Result<()> {
    let echo_handle = task::spawn(echo_server("127.0.0.1:19999"));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let proxy_handle = task::spawn(proxy_server("127.0.0.1:18989", "127.0.0.1:19999"));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut client = TcpStream::connect("127.0.0.1:18989").await?;
    {
        let msg = b"Hello, From Client!";
        client.write_all(msg).await?;

        let mut buf = vec![0; msg.len()];
        client.read_exact(&mut buf).await?;
        assert_eq!(&buf, msg);
    }

    {
        let msg = b"Goodbye, From Client!";
        client.write_all(msg).await?;

        let mut buf = vec![0; msg.len()];
        client.read_exact(&mut buf).await?;
        assert_eq!(&buf, msg);
    }

    echo_handle.abort();
    proxy_handle.abort();

    Ok(())
}

macro_rules! generate_large_data_tests {
    ($($name:ident, $size:expr, $listen_addr:expr, $forward_addr:expr);* $(;)?) => {
        $(
            #[tokio::test]
            async fn $name() -> Result<()> {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                use tokio::task;
                use tokio::net::TcpStream;

                let echo_handle = task::spawn(echo_server($forward_addr));
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                let proxy_handle = task::spawn(proxy_server($listen_addr, $forward_addr));
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                let client = TcpStream::connect($listen_addr).await?;
                let (mut reader, mut writer) = client.into_split();

                let chunk_size = 4 * 1024; // 4KB
                let data = vec![b'A'; chunk_size];

                let sender_task = task::spawn(async move {
                    let mut total_sent = 0;
                    while total_sent < $size {
                        writer.write_all(&data).await.expect("Send error");
                        writer.flush().await.expect("Flush error");
                        total_sent += chunk_size;
                    }
                });

                let mut total_received = 0;
                let mut recv_buffer = vec![0; chunk_size];

                while total_received < $size {
                    let n = reader.read(&mut recv_buffer).await?;
                    if n == 0 {
                        break;
                    }
                    total_received += n;
                    assert_eq!(&recv_buffer[..n], vec![b'A'; n]);
                }

                sender_task.await.expect("Sender task failed");
                assert_eq!($size, total_received);

                echo_handle.abort();
                proxy_handle.abort();

                Ok(())
            }
        )*
    };
}

generate_large_data_tests!(
    test_transfer_64mb, 64 * 1024 * 1024, "127.0.0.1:10001", "127.0.0.1:20001";
    test_transfer_128mb, 128 * 1024 * 1024, "127.0.0.1:10002", "127.0.0.1:20002";
    test_transfer_256mb, 256 * 1024 * 1024, "127.0.0.1:10003", "127.0.0.1:20003";
    test_transfer_512mb, 512 * 1024 * 1024, "127.0.0.1:10004", "127.0.0.1:20004";
);
