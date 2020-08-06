#![feature(wake_trait)]
#![feature(future_readiness_fns)]
#![feature(async_closure)]
#![feature(duration_zero)]

#[macro_use]
mod types;
mod async_buf_reader;
mod async_file;
mod async_sleep;
mod async_tcp_listener;
mod epoll;
mod executor;
mod listen_socket;

use crate::async_buf_reader::AsyncBufReader;
use crate::async_file::AsyncFile;
use crate::async_sleep::async_sleep;
use crate::async_tcp_listener::AsyncTcpListener;
use crate::executor::Executor;
use crate::types::*;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = Executor::new()?;
    executor.spawn(async {
        let mut async_tcp_listener = AsyncTcpListener::bind(([0, 0, 0, 0], 1234)).unwrap();
        loop {
            let socket = async_tcp_listener.incoming().await.unwrap();
            Executor::current()
                .spawn(async move {
                    println!("Client {} connected", socket);
                    let result: BoxResult<()> = (async move || {
                        let mut async_file = AsyncFile::from_fd(socket);
                        while let Some(line) = drop_temporaries!(
                            AsyncBufReader::new(&mut async_file).read_line().await?
                        ) {
                            println!("Client {} says: {}", socket, &line);
                            match &line.split(' ').collect::<Vec<_>>()[..] {
                                ["quit"] => break,
                                ["echo", value] => async_writeln!(async_file, "{}", value).await?,
                                ["sleep", value] => {
                                    async_sleep(Duration::from_millis(value.parse()?)).await;
                                    async_writeln!(async_file, "done sleeping").await?;
                                }
                                _ => async_writeln!(async_file, "unknown command").await?,
                            }
                        }
                        println!("Client {} disconnected", socket);
                        Ok(())
                    })()
                    .await;
                    if let Err(err) = result {
                        println!("Client {} errored: {:?}", socket, err);
                    }
                })
                .unwrap();
        }
    })?;
    executor.run()?;
    Ok(())
}
