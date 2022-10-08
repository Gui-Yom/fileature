use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::{fs, io, net, sync, thread};
use std::collections::{HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use eframe::{App, Frame, NativeOptions};
use eframe::egui::{CentralPanel, Context, Response, ScrollArea, Ui, Widget};

fn main() {
    eframe::run_native("Transfert", NativeOptions {
        drag_and_drop_support: true,
        ..Default::default()
    }, Box::new(|cc| {
        let sig = Arc::new(AtomicBool::default());
        thread::spawn(|| {
            let server = net::TcpListener::bind((Ipv4Addr::new(0, 0, 0, 0), 1337)).expect("Can't start server");
            thread::scope(|scope| {
                loop {
                    match server.accept() {
                        Ok((mut client, addr)) => {
                            scope.spawn(move || {
                                client.write_all(b"files").unwrap();
                                let mut ok = [0; 2];
                                client.read_exact(&mut ok).unwrap();
                            });
                        }
                        Err(e) => {
                            println!("Failed to accept connection : {e}");
                        }
                    }
                }
            });
        });

        Box::new(TransfertApp {
            shared: Vec::new(),
            downloader: Vec::new(),
            quit_signal: sig,
        })
    }));
}

struct TransfertApp {
    shared: Vec<Shared>,
    downloader: DownloaderHandle,
    quit_signal: Arc<AtomicBool>,
}

impl App for TransfertApp {
    fn update(&mut self, ctx: &Context, frame: &mut Frame) {
        for f in ctx.input().raw.dropped_files.iter() {
            if let Some(path) = &f.path {
                if path.is_file() {
                    self.shared.push(Shared {
                        path: path.clone(),
                        size: fs::metadata(path).expect("Can't read file metadata").len(),
                        cancel: false,
                    });
                } else {
                    println!("Not a file !");
                }
            } else {
                println!("Dropped file with no path ...");
            }
        }

        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                if !self.shared.is_empty() {
                    ui.label("Sharing files :");
                    for s in &mut self.shared {
                        s.show(ui);
                    }
                    self.shared.retain(|s| !s.cancel);
                    ui.separator();
                }
                for t in &self.downloader {
                    t.show(ui);
                }
                self.downloader.retain(|t| !t.cancelled());
            });
        });
    }
}

#[derive(Debug)]
struct Shared {
    path: PathBuf,
    size: u64,
    cancel: bool,
}

impl Shared {
    fn show(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{}", s.path.display()));
            if ui.button("X Remove").clicked() {
                self.cancel = true;
            }
        });
    }
}

#[derive(Debug)]
struct Download {}

/// Downloader thread
#[derive(Debug)]
struct Downloader {
    /// Cancel signal
    stop: AtomicBool,
    queue: Mutex<VecDeque<Download>>,
    bufr: Box<[u8; 8*1024*1024]>,
}

impl Downloader {
    fn new(path: impl Into<PathBuf>) -> DownloaderHandle {
        let d = Arc::new(Self { stop: AtomicBool::default() });
        let dc = d.clone();
        thread::spawn(move || {
            let mut s = net::TcpStream::connect((Ipv4Addr::new(127, 0, 0, 1), 1337)).unwrap();
            while !d.stop.load(Ordering::Relaxed) {
                println!("Downloading !");
                thread::sleep(Duration::from_secs(1));
            }
        });
        DownloaderHandle(d)
    }
}

#[derive(Debug, Clone)]
struct DownloaderHandle(pub Arc<Downloader>);

impl DownloaderHandle {
    fn cancelled(&self) -> bool {
        self.0.lock().unwrap().cancel
    }

    fn show(&self, ui: &mut Ui) {
        self.0.lock().unwrap().show(ui);
    }
}
