use std::thread;

use nvim_oxi::{
    api::{self, echo, opts::*, types::*, Buffer, Window},
    Result,
};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, SecureTCPTransport,
    TwitchIRCClient,
};

#[tokio::main(flavor = "current_thread")]
pub async fn connect(buf: Buffer) -> Result<()> {
    let win: Option<Window> = None;

    let mut plugin = Plugin {
        buffer: buf,
        window: win,
    };

    let config = ClientConfig::default();
    let (mut incoming_messages, client) =
        TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            match message {
                ServerMessage::Privmsg(msg) => {
                    let mut split = msg.message_text.splitn(2, " ");

                    if let Some("!nvim") = split.next() {
                        if let Some(text) = split.next() {
                            let name = msg.sender.name;

                            plugin.show_msg(&name, &text).unwrap_or_else(|_| {
                                plugin.echo("Plugin Error").unwrap();
                            });
                        }
                    }
                }
                _ => (),
            }
        }
    });

    client.join("mikerimebot".to_owned()).unwrap_or_else(|e| {
        println!("{:?}", e);
    });

    join_handle.await.unwrap();

    Ok(())
}

struct Plugin {
    buffer: Buffer,
    window: Option<Window>,
}

impl Plugin {
    fn echo(&self, str: &str) -> Result<()> {
        let opts = EchoOpts::builder().build();
        echo([(str, None)], false, &opts)?;
        Ok(())
    }

    fn open_win(&mut self) -> Result<()> {
        let opts = OptionOpts::builder()
            .scope(api::opts::OptionScope::Global)
            .build();

        let cols = api::get_option_value::<u32>("columns", &opts)?;
        let rows = api::get_option_value::<u32>("lines", &opts)?;

        let width: u32 = 40;
        let height: u32 = 10;

        let x: f32 = ((cols / 2) - (width - 2) / 2) as f32;
        let y: f32 = ((rows / 2) - (height - 2) / 2) as f32;

        let config = WindowConfig::builder()
            .relative(WindowRelativeTo::Editor)
            .border(nvim_oxi::api::types::WindowBorder::Rounded)
            .style(nvim_oxi::api::types::WindowStyle::Minimal)
            .height(height)
            .width(width)
            .col(x)
            .row(y)
            .focusable(true)
            .build();

        let window = nvim_oxi::api::open_win(&self.buffer, false, &config)?;
        api::set_current_win(&window)?;

        self.window = Some(window);

        Ok(())
    }

    fn show_msg(&mut self, author: &str, message: &str) -> Result<()> {
        self.buffer.set_lines(0..10, false, [author, "", message])?;

        if self.window.is_some() {
            if let Some(win) = &self.window {
                if !win.is_valid() {
                    self.open_win()?;
                }
            }
        } else {
            self.open_win()?;
        }

        Ok(())
    }
}

#[nvim_oxi::plugin]
pub fn nvim_plugin() -> Result<()> {
    let buf = nvim_oxi::api::create_buf(false, true)?;

    thread::spawn(move || {
        connect(buf).unwrap_or_else(|e| {
            println!("{:?}", e);
        });
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    #[nvim_oxi::test]
    fn it_works() {
        nvim_oxi::api::set_var("foo", 42).unwrap();
        assert_eq!(nvim_oxi::api::get_var("foo"), Ok(42));
    }
}
