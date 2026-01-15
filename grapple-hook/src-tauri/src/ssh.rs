use std::{
    io::{Read, Write},
    net::ToSocketAddrs,
    path::Path,
};

use anyhow::{anyhow, Result};

pub struct SSHSession {
    session: ssh2::Session,
}

impl SSHSession {
    pub async fn connect(
        addr: impl ToSocketAddrs,
        user: &str,
        password: &str,
    ) -> Result<SSHSession> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or(anyhow!("Invalid Address"))?;
        let user = user.to_owned();
        let password = password.to_owned();

        tokio::task::spawn_blocking(move || {
            let tcp = std::net::TcpStream::connect(addr)?;
            let mut session = ssh2::Session::new()?;
            session.set_tcp_stream(tcp);
            session.handshake()?;
            session.userauth_password(&user, &password)?;

            Ok::<SSHSession, anyhow::Error>(SSHSession { session })
        })
        .await?
    }

    pub async fn run(&self, command: &str) -> Result<CommandResult> {
        let command = command.to_owned();
        let session = self.session.clone();

        tokio::task::spawn_blocking(move || {
            let mut channel = session.channel_session()?;
            channel.exec(&command)?;

            let mut s = String::new();
            channel.read_to_string(&mut s)?;
            channel.wait_close()?;

            Ok(CommandResult {
                output: s,
                code: Some(channel.exit_status()?),
            })
        })
        .await?
    }

    pub async fn copy(&self, file: Vec<u8>, dest: &str) -> Result<()> {
        let session = self.session.clone();
        let dest = dest.to_owned();

        tokio::task::spawn_blocking(move || {
            let mut remote_file =
                session.scp_send(Path::new(&dest), 0o755, file.len() as u64, None)?;
            for chunk in file.chunks(1024) {
                remote_file.write(chunk)?;
            }
            remote_file.send_eof()?;
            remote_file.wait_eof()?;
            remote_file.close()?;
            remote_file.wait_close()?;
            Ok(())
        })
        .await?
    }
}

#[derive(Clone, Debug)]
pub struct CommandResult {
    output: String,
    code: Option<i32>,
}

impl CommandResult {
    pub fn output(&self) -> String {
        self.output.clone()
    }

    pub fn success(&self) -> bool {
        self.code() == Some(0)
    }

    pub fn code(&self) -> Option<i32> {
        self.code
    }
}
