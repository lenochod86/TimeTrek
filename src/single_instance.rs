//! Jediná instance aplikace. První spuštění naslouchá na lokální smyčce; druhé se připojí,
//! pošle „show“ a skončí, aby běžící instance otevřela okno. Funguje stejně na všech systémech.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

const PORT: u16 = 47531;
const HELLO: &str = "timetrek";

fn addr() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, PORT))
}

/// Vrátí `true`, jestli jsme první instance (a tedy máme pokračovat). `on_show` se zavolá
/// z jiného vlákna, když se někdo pokusí spustit druhou instanci.
pub fn acquire(on_show: impl Fn() + Send + 'static) -> bool {
    match TcpListener::bind(addr()) {
        Ok(listener) => {
            let _ = std::thread::Builder::new().name("timetrek-single".into()).spawn(move || {
                for stream in listener.incoming().flatten() {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                    let mut s = stream;
                    let _ = writeln!(s, "{HELLO}");
                    let mut line = String::new();
                    if BufReader::new(&s).read_line(&mut line).is_ok() && line.trim() == "show" {
                        on_show();
                    }
                }
            });
            true
        }
        Err(_) => !notify_running(),
    }
}

/// Zeptá se běžící instance; `true`, když odpověděla, že je TimeTrek.
fn notify_running() -> bool {
    let Ok(mut s) = TcpStream::connect_timeout(&addr(), Duration::from_millis(500)) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_millis(500)));
    let mut hello = String::new();
    if BufReader::new(&s).read_line(&mut hello).is_err() || hello.trim() != HELLO {
        return false;
    }
    let _ = writeln!(s, "show");
    true
}
