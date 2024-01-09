use crawl::downloader::{Downloader, ResThreadArg, ResMessage, get_res_thread_arg, start_crawl};
use select::predicate::Name;
use select::document::Document;
use url::Url;
use bytes::Bytes;
use encoding_rs::UTF_8;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;
use anyhow;


struct Data{
    title: String,
    url: String,
}

impl Data{
    fn new(title: &str, url: &str) -> Data{
        Data{
            title: title.to_string(),
            url: url.to_string()
        }
    }
}
struct Manager{
    datas: Vec<Data>,
    added_urls: HashSet<String>,
}

fn decode_bytes(data:&Bytes) -> String{
    let (text, _, _) = UTF_8.decode(data.as_ref());
    text.into_owned()
}
fn parse(msg:ResMessage<Option<()>>,arg:&ResThreadArg<Option<()>>,manager: &Arc<Mutex<Manager>>) -> anyhow::Result<()>{
    let d;
    match &msg.data {
        Ok(data)=> match data{
            Some(v) => d = decode_bytes(&v),
            None => return Ok(()),
        },
        Err(_)=>return msg.retry(false)
    }
    let doc = Document::from(d.as_str());
    let base_url = Url::parse(msg.url.as_str())?;
    let title = doc.find(Name("title")).next().unwrap().text();
    {
        let mut m = manager.lock().unwrap();
        m.datas.push(Data::new(&title, base_url.as_str()));
    }
    
    for node in doc.find(Name("a")){
        let href = node.attr("href");
        match href{
            None=>{},
            Some(h)=>{
                let no_hash = match h.split_once("#"){
                    Some((t, _)) => t,
                    None => h
                };
                let new_url = base_url.join(no_hash)?.to_string();
                {
                    let mut m = manager.lock().unwrap();
                    if !m.added_urls.contains(&new_url)
                    {
                        m.added_urls.insert(new_url.clone());
                        arg.start_url(new_url, false, Arc::new(None))?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn res_run(arg:ResThreadArg<Option<()>>, manager: Arc<Mutex<Manager>>){
    loop {
        match arg.get_msg(){
            Ok(msg)=>match parse(msg, &arg, &manager){
                Ok(_)=>{},
                Err(_)=>{}
            },
            Err(_)=>{}
        }
    }

}

fn main() -> anyhow::Result<()> {
    let download = Arc::new(Downloader::new(
        String::from(r"data/book1"),
        String::from("https://doc.rust-lang.org/book/")
    ));
    let url = String::from("https://doc.rust-lang.org/book/index.html");
    let manager = Arc::new(Mutex::new(Manager{datas:Vec::new(), added_urls:HashSet::from([url.clone()])}));
    download.start_url(url, false, Arc::new(None))?;
    for _ in 0..16{
        let res_arg = get_res_thread_arg(&download);
        let r = Arc::clone(&manager);
        thread::spawn(move || res_run(res_arg, r));
    }
    start_crawl(&download, 16);
    download.wait_finish();
    {
        let m = manager.lock().unwrap();
        println!("finish {}", m.datas.len());
        for item in m.datas.iter(){
            println!("{} {}", item.url, item.title);
        }
    }
    Ok(())
}
