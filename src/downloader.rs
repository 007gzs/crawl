use std::fs::File;  
use std::io::{Write, Read};
use std::sync::Arc; 
use std::thread::{self, sleep};
use std::time::Duration; 
use bytes::Bytes;
use std::fs;
use anyhow;  
use std::path::Path;
use reqwest;
use flume::{Sender, Receiver};
use std::sync::atomic::{AtomicUsize, Ordering};

struct ReqMessage<E>{
    url:String,
    force: bool,
    flag: Arc<E>,
}
pub struct ResMessage<E>{
    pub url:String,
    pub data: anyhow::Result<Option<Bytes>>,
    pub flag: Arc<E>,
    downloader:Arc<Downloader<E>>
}
impl<E> ReqMessage<E>{
    fn gen_res(&self, data: anyhow::Result<Option<Bytes>>, downloader:&Arc<Downloader<E>>) -> ResMessage<E>{
        ResMessage{
            url: self.url.clone(),
            data: data,
            flag: Arc::clone(&self.flag),
            downloader: Arc::clone(downloader),
        }
    }
}
impl<E> Drop for ResMessage<E>{
    fn drop(&mut self){
        self.downloader.parse_num.fetch_add(1, Ordering::Relaxed);
        self.downloader.end_index.fetch_add(1, Ordering::Relaxed);
    }
}
#[derive(Clone)]
pub struct Downloader<E>{
    root_path: String,
    base_url: String,
    start_index: Arc<AtomicUsize>,
    end_index: Arc<AtomicUsize>,
    download_num: Arc<AtomicUsize>,
    connect_num: Arc<AtomicUsize>,
    parse_num: Arc<AtomicUsize>,
    req_sender:Sender<ReqMessage<E>>, 
    req_receiver:Receiver<ReqMessage<E>>, 
    res_sender:Sender<ResMessage<E>>, 
    res_receiver:Receiver<ResMessage<E>>
}

struct ReqThreadArg<E>{
    receiver:Receiver<ReqMessage<E>>, 
    sender: Sender<ResMessage<E>>

}
pub struct ResThreadArg<E>{
    receiver:Receiver<ResMessage<E>>, 
    sender: Sender<ReqMessage<E>>,
    downloader:Arc<Downloader<E>>
}

fn req_run<E: Send + Sync + 'static>(arg: ReqThreadArg<E>, downloader:Arc<Downloader<E>>){
    loop{
        match arg.receiver.recv() {
            Ok(msg) => {
                downloader.start_index.fetch_add(1, Ordering::Relaxed);
                let data = downloader.download(msg.url.clone(), msg.force);
                downloader.download_num.fetch_add(1, Ordering::Relaxed);
                match arg.sender.send(msg.gen_res(data, &downloader)){
                    Ok(_)=>{},
                    Err(_)=>{}
                };
                downloader.end_index.fetch_add(1, Ordering::Relaxed);
            },
            Err(_) => {},
        }
    }
}

impl<E: Send + Sync + 'static>  Downloader<E>
{
    pub fn new(root_path: String, base_url: String) -> Downloader<E>{
        let (req_sender, req_receiver) = flume::unbounded();
        let (res_sender, res_receiver) = flume::unbounded();
        Downloader{
            root_path: root_path,
            base_url: base_url,
            start_index:Arc::new(AtomicUsize::new(0)),
            end_index:Arc::new(AtomicUsize::new(0)),
            download_num:Arc::new(AtomicUsize::new(0)),
            connect_num:Arc::new(AtomicUsize::new(0)),
            parse_num:Arc::new(AtomicUsize::new(0)),
            req_sender: req_sender,
            req_receiver: req_receiver,
            res_sender:res_sender,
            res_receiver: res_receiver,
        }
    }
    fn connect_real(&self, url:String, proxy:Option<reqwest::Proxy>) -> anyhow::Result<Bytes>{
        let mut builder = reqwest::blocking::Client::builder();
        match proxy {
            Some(p)=>{
                builder = builder.proxy(p);
            },
            None=>{}
        }
        Ok(builder.build()?.get(url).send()?.bytes()?)
    }
    
    fn download(&self, url:String, force:bool) -> anyhow::Result<Option<Bytes>>{
        if url.len() < self.base_url.len() || url[0..self.base_url.len()] != self.base_url{
            return Ok(None);
        }
        let path = Path::join(Path::new(self.root_path.as_str()), url.as_str().chars().skip(self.base_url.len()).collect::<String>());
        match path.parent(){
            Some(p) =>  fs::create_dir_all(p)?,
            None => {},
        };
        if !force {
            match File::open(&path){
                Ok(mut file) => {
                    let size = file.metadata().map(|m| m.len() as usize).ok().unwrap_or(0);
                    let mut buffer = Vec::with_capacity(size);
                    file.read_to_end(&mut buffer)?;
                    return Ok(Some(Bytes::from(buffer)));
                },
                Err(_) =>{}
            }
        }

        let body = self.connect_real(url.clone(), None)?;
        self.connect_num.fetch_add(1, Ordering::Relaxed);
        let mut file = File::create(path)?;
        for chunk in body.chunks(4096){
            file.write(chunk)?;
        }
        Ok(Some(body))
    }
    pub fn wait_finish(&self){
        loop{
            sleep(Duration::from_secs(1));
            let end_index = self.end_index.load(Ordering::Relaxed);
            if !self.req_sender.is_empty() || !self.res_sender.is_empty(){
                continue
            }
            if self.start_index.load(Ordering::Relaxed) == end_index{
                break
            }
        }
    }
    pub fn start_url(&self, url:String, force:bool, url_flag: Arc<E>)-> anyhow::Result<()>{
        let msg = ReqMessage{url: url, force: force, flag: url_flag};
        self.req_sender.send(msg)?;
        Ok(())
    }
}

impl<E: Send + Sync + 'static> ResMessage<E>{
    pub fn retry(&self, force:bool)-> anyhow::Result<()>{
        self.downloader.start_url(self.url.clone(), force, Arc::clone(&self.flag))
    }
}
pub fn start_crawl<E:Send + Sync + 'static>(downloader:&Arc<Downloader<E>>, thread_num:u16){
    for _ in 0..thread_num {
        let d = Arc::clone(&downloader);
        let t = ReqThreadArg{receiver: downloader.req_receiver.clone(), sender: downloader.res_sender.clone()};
        thread::spawn(move || req_run(t, d));
    }
}

pub fn get_res_thread_arg<E>(downloader: &Arc<Downloader<E>>) -> ResThreadArg<E>{
    let sender: Sender<ReqMessage<E>> = downloader.req_sender.clone();
    let receiver = downloader.res_receiver.clone();
    ResThreadArg{receiver: receiver, sender: sender, downloader:Arc::clone(downloader)}
}

impl<E: Send + Sync + 'static > ResThreadArg<E>{
    pub fn start_url(&self, url:String, force:bool, url_flag: Arc<E>)-> anyhow::Result<()>{
        let msg = ReqMessage{url: url, force: force, flag: url_flag};
        self.sender.send(msg)?;
        Ok(())
    }

    pub fn get_msg(&self) -> anyhow::Result<ResMessage<E>>{
        let msg = self.receiver.recv()?;
        self.downloader.start_index.fetch_add(1, Ordering::Relaxed);
        Ok(msg)
    }
}
