# tokio-splice

Implemented splice(2) based bidirectional data transmission in tokio-rs.
Just like [`tokio::io::copy_bidirectional`](https://docs.rs/tokio/latest/tokio/io/fn.copy_bidirectional.html).

## Example

The following code implements a TCP proxy that forwards traffic on port 8989 to example.com.

```Rust
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
```

## Benchmark

10 VUs, 10 seconds, work as a http proxy to download 100Mb file.

### `tokio::io::copy_bidirectional`

```
     data_received..................: 13 GB 1.2 GB/s
     data_sent......................: 14 kB 1.3 kB/s
     http_req_blocked...............: avg=209.7µs  min=94.42µs  med=171.66µs max=707.76µs p(90)=335.38µs p(95)=444.93µs
     http_req_connecting............: avg=131.44µs min=55.73µs  med=105.87µs max=466.25µs p(90)=216.11µs p(95)=284.09µs
     http_req_duration..............: avg=803.62ms min=310.7ms  med=811.17ms max=960.74ms p(90)=873.55ms p(95)=902.29ms
       { expected_response:true }...: avg=803.62ms min=310.7ms  med=811.17ms max=960.74ms p(90)=873.55ms p(95)=902.29ms
     http_req_failed................: 0.00% ✓ 0         ✗ 128
     http_req_receiving.............: avg=740.73ms min=255.61ms med=739.93ms max=955.05ms p(90)=813.65ms p(95)=878.69ms
     http_req_sending...............: avg=47.54µs  min=18.85µs  med=41.38µs  max=168.45µs p(90)=71.9µs   p(95)=83.85µs
     http_req_tls_handshaking.......: avg=0s       min=0s       med=0s       max=0s       p(90)=0s       p(95)=0s
     http_req_waiting...............: avg=62.84ms  min=973.15µs med=65.64ms  max=107.5ms  p(90)=79.21ms  p(95)=83.39ms
     http_reqs......................: 128   12.272712/s
     iteration_duration.............: avg=803.98ms min=310.95ms med=811.51ms max=961.06ms p(90)=873.97ms p(95)=902.63ms
     iterations.....................: 128   12.272712/s
     vus............................: 10    min=10      max=10
     vus_max........................: 10    min=10      max=10
```

<details>
    <summary>Full process statistics with <code>pidstat -r -w -u -d -p</code></summary>
    
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:11 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:11 PM  1000     38138    0.00    0.00    0.00    0.00    0.01    14  proxy

    11:48:11 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:11 PM  1000     38138      6.56      0.00    3840    2540   0.01  proxy

    11:48:11 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:11 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:11 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:11 PM  1000     38138      0.01      0.00  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:12 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:12 PM  1000     38138    0.01    0.02    0.00    0.00    0.02    10  proxy

    11:48:12 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:12 PM  1000     38138      6.60      0.00    3976    2668   0.01  proxy

    11:48:12 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:12 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:12 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:12 PM  1000     38138      0.01      0.01  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:13 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:13 PM  1000     38138    0.02    0.14    0.00    0.00    0.16    10  proxy

    11:48:13 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:13 PM  1000     38138      6.60      0.00    3976    2668   0.01  proxy

    11:48:13 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:13 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:13 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:13 PM  1000     38138      0.01      0.04  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:14 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:14 PM  1000     38138    0.03    0.25    0.00    0.00    0.29    13  proxy

    11:48:14 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:14 PM  1000     38138      6.59      0.00    3976    2668   0.01  proxy

    11:48:14 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:14 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:14 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:14 PM  1000     38138      0.01      0.06  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:15 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:15 PM  1000     38138    0.05    0.37    0.00    0.00    0.42     9  proxy

    11:48:15 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:15 PM  1000     38138      6.58      0.00    3976    2668   0.01  proxy

    11:48:15 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:15 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:15 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:15 PM  1000     38138      0.01      0.10  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:16 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:16 PM  1000     38138    0.06    0.49    0.00    0.00    0.55     3  proxy

    11:48:16 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:16 PM  1000     38138      6.57      0.00    3976    2668   0.01  proxy

    11:48:16 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:16 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:16 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:16 PM  1000     38138      0.01      0.22  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:17 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:17 PM  1000     38138    0.08    0.61    0.00    0.00    0.68     5  proxy

    11:48:17 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:17 PM  1000     38138      6.56      0.00    3976    2668   0.01  proxy

    11:48:17 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:17 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:17 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:17 PM  1000     38138      0.01      0.27  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:18 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:18 PM  1000     38138    0.09    0.73    0.00    0.00    0.82     4  proxy

    11:48:18 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:18 PM  1000     38138      6.55      0.00    3976    2668   0.01  proxy

    11:48:18 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:18 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:18 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:18 PM  1000     38138      0.01      0.30  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:19 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:19 PM  1000     38138    0.10    0.84    0.00    0.00    0.95     5  proxy

    11:48:19 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:19 PM  1000     38138      6.54      0.00    3976    2668   0.01  proxy

    11:48:19 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:19 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:19 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:19 PM  1000     38138      0.01      0.34  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:20 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:20 PM  1000     38138    0.12    0.96    0.00    0.00    1.08    14  proxy

    11:48:20 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:20 PM  1000     38138      6.54      0.00    3976    2668   0.01  proxy

    11:48:20 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:20 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:20 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:20 PM  1000     38138      0.01      0.36  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:21 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:21 PM  1000     38138    0.13    1.07    0.00    0.00    1.21     6  proxy

    11:48:21 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:21 PM  1000     38138      6.53      0.00    3976    2668   0.01  proxy

    11:48:21 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:21 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:21 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:21 PM  1000     38138      0.01      0.41  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:22 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:22 PM  1000     38138    0.15    1.19    0.00    0.00    1.34     4  proxy

    11:48:22 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:22 PM  1000     38138      6.52      0.00    3976    2668   0.01  proxy

    11:48:22 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:22 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:22 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:22 PM  1000     38138      0.01      0.45  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:48:23 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:48:23 PM  1000     38138    0.15    1.22    0.00    0.00    1.37     5  proxy

    11:48:23 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:48:23 PM  1000     38138      6.51      0.00    3976    2668   0.01  proxy

    11:48:23 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:48:23 PM  1000     38138      0.00      0.05      0.00       0  proxy

    11:48:23 PM   UID       PID   cswch/s nvcswch/s  Command
    11:48:23 PM  1000     38138      0.01      0.47  proxy

</details>

### `zero_copy_bidirectional`

```
     data_received..................: 21 GB 2.0 GB/s
     data_sent......................: 22 kB 2.2 kB/s
     http_req_blocked...............: avg=1.04ms   min=120.68µs med=595.6µs  max=41.71ms  p(90)=1.33ms   p(95)=1.92ms
     http_req_connecting............: avg=614.87µs min=70.12µs  med=388.35µs max=10.87ms  p(90)=852.66µs p(95)=1.24ms
     http_req_duration..............: avg=487.26ms min=292.38ms med=469.17ms max=844.43ms p(90)=621.37ms p(95)=692.62ms
       { expected_response:true }...: avg=487.26ms min=292.38ms med=469.17ms max=844.43ms p(90)=621.37ms p(95)=692.62ms
     http_req_failed................: 0.00% ✓ 0         ✗ 208
     http_req_receiving.............: avg=451.06ms min=265.68ms med=431.16ms max=839.83ms p(90)=572.64ms p(95)=684.98ms
     http_req_sending...............: avg=743.45µs min=29.89µs  med=129.48µs max=44.06ms  p(90)=253.81µs p(95)=322.91µs
     http_req_tls_handshaking.......: avg=0s       min=0s       med=0s       max=0s       p(90)=0s       p(95)=0s
     http_req_waiting...............: avg=35.45ms  min=352.97µs med=34.74ms  max=97.29ms  p(90)=64.27ms  p(95)=73.75ms
     http_reqs......................: 208   20.300179/s
     iteration_duration.............: avg=489.25ms min=293.18ms med=470.63ms max=845.03ms p(90)=622.59ms p(95)=693.61ms
     iterations.....................: 208   20.300179/s
     vus............................: 10    min=10      max=10
     vus_max........................: 10    min=10      max=10

```


<details>
    <summary>Full process statistics with <code>pidstat -r -w -u -d -p</code></summary>
    
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:23 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:23 PM  1000     59138    0.00    0.00    0.00    0.00    0.00     4  proxy

    11:49:23 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:23 PM  1000     59138      6.52      0.00    3836    2432   0.01  proxy

    11:49:23 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:23 PM  1000     59138      0.00     27.11      0.01       0  proxy

    11:49:23 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:23 PM  1000     59138      0.01      0.00  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:24 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:24 PM  1000     59138    0.00    0.01    0.00    0.00    0.01     1  proxy

    11:49:24 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:24 PM  1000     59138      6.52      0.00    3836    2432   0.01  proxy

    11:49:24 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:24 PM  1000     59138      0.00     27.08      0.01       0  proxy

    11:49:24 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:24 PM  1000     59138      0.06      0.04  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:25 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:25 PM  1000     59138    0.02    0.08    0.00    0.01    0.10     2  proxy

    11:49:25 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:25 PM  1000     59138      6.51      0.00    3836    2432   0.01  proxy

    11:49:25 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:25 PM  1000     59138      0.00     27.04      0.01       0  proxy

    11:49:25 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:25 PM  1000     59138      0.53      0.23  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:26 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:26 PM  1000     59138    0.04    0.18    0.00    0.01    0.21     1  proxy

    11:49:26 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:26 PM  1000     59138      6.50      0.00    3836    2432   0.01  proxy

    11:49:26 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:26 PM  1000     59138      0.00     27.01      0.01       0  proxy

    11:49:26 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:26 PM  1000     59138      0.80      0.37  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:27 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:27 PM  1000     59138    0.05    0.28    0.00    0.01    0.32    12  proxy

    11:49:27 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:27 PM  1000     59138      6.49      0.00    3836    2432   0.01  proxy

    11:49:27 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:27 PM  1000     59138      0.00     26.98      0.01       0  proxy

    11:49:27 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:27 PM  1000     59138      1.67      0.58  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:28 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:28 PM  1000     59138    0.06    0.37    0.00    0.02    0.44    12  proxy

    11:49:28 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:28 PM  1000     59138      6.49      0.00    3836    2432   0.01  proxy

    11:49:28 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:28 PM  1000     59138      0.00     26.94      0.01       0  proxy

    11:49:28 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:28 PM  1000     59138      1.77      0.82  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:29 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:29 PM  1000     59138    0.08    0.46    0.00    0.03    0.54    12  proxy

    11:49:29 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:29 PM  1000     59138      6.48      0.00    3836    2432   0.01  proxy

    11:49:29 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:29 PM  1000     59138      0.00     26.91      0.01       0  proxy

    11:49:29 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:29 PM  1000     59138      2.38      1.43  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:30 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:30 PM  1000     59138    0.09    0.56    0.00    0.04    0.65    12  proxy

    11:49:30 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:30 PM  1000     59138      6.47      0.00    3836    2432   0.01  proxy

    11:49:30 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:30 PM  1000     59138      0.00     26.88      0.01       0  proxy

    11:49:30 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:30 PM  1000     59138      2.97      1.71  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:31 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:31 PM  1000     59138    0.10    0.65    0.00    0.05    0.75     6  proxy

    11:49:31 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:31 PM  1000     59138      6.46      0.00    3836    2432   0.01  proxy

    11:49:31 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:31 PM  1000     59138      0.00     26.85      0.01       0  proxy

    11:49:31 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:31 PM  1000     59138      3.71      1.96  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:32 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:32 PM  1000     59138    0.12    0.75    0.00    0.05    0.87    12  proxy

    11:49:32 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:32 PM  1000     59138      6.45      0.00    3836    2432   0.01  proxy

    11:49:32 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:32 PM  1000     59138      0.00     26.81      0.01       0  proxy

    11:49:32 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:32 PM  1000     59138      3.82      2.08  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:33 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:33 PM  1000     59138    0.13    0.84    0.00    0.06    0.98     6  proxy

    11:49:33 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:33 PM  1000     59138      6.45      0.00    3836    2432   0.01  proxy

    11:49:33 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:33 PM  1000     59138      0.00     26.78      0.01       0  proxy

    11:49:33 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:33 PM  1000     59138      3.99      2.39  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:34 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:34 PM  1000     59138    0.14    0.93    0.00    0.06    1.07     8  proxy

    11:49:34 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:34 PM  1000     59138      6.44      0.00    3836    2432   0.01  proxy

    11:49:34 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:34 PM  1000     59138      0.00     26.75      0.01       0  proxy

    11:49:34 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:34 PM  1000     59138      4.35      2.50  proxy
    Linux 6.2.13-arch1-1 (misaka) 	05/03/2023 	_x86_64_	(16 CPU)

    11:49:35 PM   UID       PID    %usr %system  %guest   %wait    %CPU   CPU  Command
    11:49:35 PM  1000     59138    0.14    0.93    0.00    0.06    1.07     5  proxy

    11:49:35 PM   UID       PID  minflt/s  majflt/s     VSZ     RSS   %MEM  Command
    11:49:35 PM  1000     59138      6.43      0.00    3836    2432   0.01  proxy

    11:49:35 PM   UID       PID   kB_rd/s   kB_wr/s kB_ccwr/s iodelay  Command
    11:49:35 PM  1000     59138      0.00     26.72      0.01       0  proxy

    11:49:35 PM   UID       PID   cswch/s nvcswch/s  Command
    11:49:35 PM  1000     59138      4.44      2.49  proxy

</detail>
