use std::io::Result;
use tokio::net::{TcpListener, TcpStream};
use tokio_splice::zero_copy_bidirectional;

async fn forwarding(mut stream1: TcpStream) -> Result<()> {
    let mut stream2 = TcpStream::connect("93.184.216.34:80").await?;
    zero_copy_bidirectional(&mut stream1, &mut stream2).await?;
    Ok(())
}

async fn serve() -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:8989").await?;

    loop {
        let (conn, addr) = listener.accept().await?;
        println!("process incoming connection from {addr}");
        tokio::spawn(forwarding(conn));
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    println!("PID is {}", std::process::id());
    tokio::select! {
        res = serve() => {
            if let Err(err) = res {
                println!("serve failed {err}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("shutting down");
        }
    }

    Ok(())
}
