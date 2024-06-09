use std::{cell::RefCell, rc::Rc, thread};

use nvim_oxi::{
    api::{self, opts::*, types::*, Buffer, Window},
    libuv::AsyncHandle,
    schedule, Result,
};
use tokio::sync::mpsc::{self, UnboundedSender};
use twitch_irc::{
    login::StaticLoginCredentials, message::ServerMessage, ClientConfig, SecureTCPTransport,
    TwitchIRCClient,
};

#[tokio::main(flavor = "current_thread")]
pub async fn connect(handle: AsyncHandle, sender: UnboundedSender<CommandPayload>) -> Result<()> {
    let config = ClientConfig::default();
    let (mut incoming_messages, client) =
        TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(config);

    let join_handle = tokio::spawn(async move {
        while let Some(message) = incoming_messages.recv().await {
            match message {
                ServerMessage::Privmsg(msg) => {
                    let mut split = msg.message_text.trim().splitn(2, " ");

                    let command = split.next();
                    let argument = split.next();

                    if let Some("!nvim") = command {
                        if let Some(text) = argument {
                            let name = msg.sender.name;

                            sender
                                .send(CommandPayload {
                                    command: Command::Message(name.to_owned(), text.to_owned()),
                                })
                                .unwrap();

                            handle.send().unwrap();
                        }
                    }

                    if let Some("!colorscheme") = command {
                        if let Some(colorscheme) = argument {
                            sender
                                .send(CommandPayload {
                                    command: Command::ColorScheme(colorscheme.to_owned()),
                                })
                                .unwrap();

                            handle.send().unwrap();
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
    fn err(&self, str: &str) -> Result<()> {
        api::err_writeln(str);
        Ok(())
    }

    fn colosrcheme(&self, colorscheme: String) -> Result<()> {
        let mut command = String::from("colorscheme ");
        command.push_str(colorscheme.as_str());

        api::command(command.as_str())?;

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

#[derive(Debug)]
pub enum Command {
    Message(String, String),
    ColorScheme(String),
}

#[derive(Debug)]
pub struct CommandPayload {
    command: Command,
}

#[nvim_oxi::plugin]
pub fn nvim_plugin() -> Result<()> {
    let (sender, mut receiver) = mpsc::unbounded_channel::<CommandPayload>();

    let buf = nvim_oxi::api::create_buf(false, true)?;

    let win: Option<Window> = None;

    let plugin: Rc<RefCell<Plugin>> = Rc::new(RefCell::new(Plugin {
        buffer: buf,
        window: win,
    }));

    let handle = AsyncHandle::new(move || {
        let payload = receiver.blocking_recv().unwrap();

        let plugin_ref = Rc::clone(&plugin);

        schedule(move |_| {
            let mut plugin = plugin_ref.borrow_mut();

            match payload.command {
                Command::Message(author, text) => {
                    plugin
                        .show_msg(author.as_str(), text.as_str())
                        .unwrap_or_else(|_| {
                            plugin.err("Plugin Error: Message").unwrap();
                        });
                }
                Command::ColorScheme(colorscheme) => {
                    plugin.colosrcheme(colorscheme).unwrap_or_else(|_| {
                        plugin.err("Plugin Error: Colorscheme").unwrap();
                    });
                }
            }
        });
    })?;

    thread::spawn(move || {
        connect(handle, sender).unwrap_or_else(|e| {
            println!("{:?}", e);
        });
    });

    Ok(())
}
