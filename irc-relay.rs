extern crate serialize;
use serialize::json;
use serialize::Decodable;


use std::io::TcpStream;
use std::io::stdio;
use std::io::BufferedReader;
use std::io::fs;
use std::io::net::unix::UnixListener;
use std::io::{Acceptor,Listener,IoError};
use std::str;
use std::comm;
use std::comm::{Select};
use std::os;


#[deriving(Decodable, Encodable, Clone)]
pub struct Config  {
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
        Some(s.slice_from(s.len()))
    } else {
        None
    }
}


fn local_ipc(conf : &Config, tx : Sender<String>) {
    let path = Path::new(conf.path.as_slice());

    // Delete old socket if necessary
    if path.exists() {
        fs::unlink(&path).unwrap();
    }

    // Bind to socket
    let stream = match UnixListener::bind(&path) {
        Err(_)     => fail!("failed to bind socket"),
        Ok(stream) => stream,
    };

    for client in stream.listen().incoming() {
        let tx = tx.clone();
        spawn(proc() {
            let mut client = client;
            match client.read_to_end() {
                Ok(ret) =>
                    match String::from_utf8(ret) {
                        Ok(msg) => {
                            stdio::println(msg.as_slice());
                            tx.send(msg);
                        },
                        _ => {}
                    },
                _ => {}
            }
        });
    }
}

fn irc_client(conf : &Config, rx : &Receiver<String>) -> Result<(), IoError> {
    let mut stream = try!(TcpStream::connect(conf.host.as_slice(), conf.port));
        
    // Connect
    try!(stream.write(format!("USER {} 0 * :{}\n", conf.user, conf.realname).as_bytes()));
    try!(stream.write(format!("NICK {}\n", conf.nick).as_bytes()));
    try!(stream.write(format!("JOIN {}\n", conf.channel).as_bytes()));
    
    let (irc_tx, irc_rx) = comm::channel();
    
    let reader = BufferedReader::new(stream.clone());
    // Receieve network
    spawn(proc() {
        let mut tmp = reader;
        loop {
            irc_tx.send(tmp.read_line());
        }
    });

    let sel = Select::new();
    let mut rx     = sel.handle(rx);
    let mut irc_rx = sel.handle(&irc_rx);
    unsafe {
        rx.add();
        irc_rx.add();
    }

    loop {
        let ret = sel.wait();
        if ret == rx.id() {
            let msg = rx.recv();
            try!(stream.write(format!("PRIVMSG {} :{}\n", conf.channel, msg).as_bytes()))
        } else if ret == irc_rx.id() {
            let line = irc_rx.recv();
            match line {
                Ok(s_) => {
                    let s = s_.as_slice().trim_right();
                    stdio::println(s);
                    match strip_prefix("PING ", s) {
                        None => (),
                        Some(ping_req) => try!(stream.write(format!("PONG {}\n", ping_req).as_bytes()))
                    }
                },
                Err(e) => { println!("error {}", e); return Err(e)}
            }
        }   
    }
}


fn get_config() -> Config {
    let path = 
        if os::args().len() >= 2 {
            Path::new(os::args().get(1).as_slice())
        } else { 
            Path::new("/etc/irc-relay.json")
        };
    
    let raw_file =
        match fs::File::open(&path).read_to_end() {
        Err(e) => {
            fail!(format!("Error opening file \"{}\": {}", path.display(), e));
        }
        Ok(x) => x
    };
   
    let raw_file_utf8 = str::from_utf8(raw_file.as_slice()).unwrap();
    let json_object = json::from_str(raw_file_utf8).unwrap();
    let mut decoder = json::Decoder::new(json_object);
    Decodable::decode(&mut decoder).unwrap()
    
}

fn main() {
    let conf = get_config();
    
    let (tx, rx) = comm::channel();
    
    let c = conf.clone();
    spawn(proc() {
        local_ipc(&c, tx);
    });

    loop {
        match irc_client(&conf, &rx) {
            Err(e) => println!("Error: {}\nReconnecting..", e),
            Ok(_)  => ()
        }
    }
}