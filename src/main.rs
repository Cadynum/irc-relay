#![feature(exit_status, std_misc, scoped)]
extern crate rustc_serialize;
extern crate unix_socket;

use std::io;
use std::io::{Read, Write, BufRead, BufReader};
use std::fs;
use std::net::TcpStream;
use std::env;
use std::error::Error;
use std::clone::Clone;
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};

use rustc_serialize::{json, Decodable};
use unix_socket::UnixListener;

#[derive(RustcDecodable, RustcEncodable)]
struct Config  {
      path     : String
    , host     : String
    , port     : u16
    , nick     : String
    , user     : String
    , realname : String
    , channel  : String
}


fn strip_prefix<'a>(prefix : &str, s : &'a str) -> Option<&'a str>{
    if s.starts_with(prefix) {
        Some(&s[s.len()..])
    } else {
        None
    }
}


fn local_ipc(conf : &Config, tx : &Sender<String>) {
    // Delete old socket if necessary
    let _ = fs::remove_file(&conf.path);

    // Bind to socket
    let stream = UnixListener::bind(&conf.path).unwrap();
    let mut id = 0usize;
    for client in stream.incoming() {
        let tx = tx.clone();
        thread::spawn(move || {
            println!("Client {} connected", id);
            let reader = BufReader::new(client.unwrap());
            for line in reader.lines() {
                let line = line.unwrap();
                if line.len() == 0 {
                    continue;
                }
                println!("{}: {}", id, line);
                tx.send(line).unwrap();
            }
            println!("Client {} disconnected", id);
        });
        id += 1;
    }
}

fn irc_client(conf : &Config, rx : &Receiver<String>) -> Result<(), io::Error> {

    let stream = try!(TcpStream::connect((&conf.host[..], conf.port)));

    // Connect
    try!(writeln!(&stream, "USER {} 0 * :{}", conf.user, conf.realname));
    try!(writeln!(&stream, "NICK {}", conf.nick));
    try!(writeln!(&stream, "JOIN {}", conf.channel));

    let (irc_tx, irc_rx) = channel::<String>();

    let reader = BufReader::new(try!(stream.try_clone()));
    // Receieve network
    let guard = thread::scoped(move || {
        for line in reader.lines() {
            irc_tx.send(line.unwrap()).unwrap();
        }
    });

    loop {
        select! (
            msg = rx.recv() => {
                try!(writeln!(&stream, "PRIVMSG {} :{}", conf.channel, msg.unwrap()));
            },
            msg = irc_rx.recv() => {
                let s = msg.unwrap();
                let s = s[..].trim_right();
                println!("{}", s);
                strip_prefix("PING ", s).and_then(|s| writeln!(&stream, "PONG {}", s).ok());
            }
        )
    }
}

fn get_config() -> Result<Config, String> {
    let path = env::args().nth(1).unwrap_or("/etc/irc-relay.json".to_owned());

    let mut raw_file_utf8 = String::new();
    let _ = try!(fs::File::open(&path)
        .map_err(|x| format!("{}: {}", path, x)))
        .read_to_string(&mut raw_file_utf8);

    let json_object = try!(json::Json::from_str(&raw_file_utf8)
        .map_err(|x| format!("{}: {}", path, x)));
    let mut decoder = json::Decoder::new(json_object);
    Decodable::decode(&mut decoder)
        .map_err(|x| format!("{}: {}", path, x))

}


fn main() {
    let conf = match get_config() {
        Ok(s) => s,
        Err(e) => {
            println!("Error reading config: {}", e);
            env::set_exit_status(1);
            return;
        }
    };
    let (tx, rx) = channel::<String>();

    let guard = thread::scoped(|| {
        let tx = tx;
        local_ipc(&conf, &tx)
    });

    loop {
        match irc_client(&conf, &rx) {
            Err(e) => println!("Error: {}\nReconnecting..", e),
            Ok(_)  => ()
        }
    }
}
