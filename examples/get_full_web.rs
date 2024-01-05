use std::error::Error as StdError;
use async_trait::async_trait;
use crawl::downloader::{Downloader, Crawler};
use futures::Future;
use select::predicate::Name;
use select::document::Document;
use url::Url;
use bytes::Bytes;
use encoding_rs::UTF_8;
use std::collections::HashSet;

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

struct CrawlerData;

fn decode_bytes(data:&Bytes) -> String{
    let (text, _, _) = UTF_8.decode(data.as_ref());
    text.into_owned()
}
#[async_trait(?Send)]
impl Crawler<Manager> for CrawlerData{
    async fn parse(&self, downloader:&mut Downloader<Manager>, url:String, data:Option<Bytes>) -> Result<(), Box<dyn StdError>> {
        let d;
        match data {
            Some(v) => d = decode_bytes(&v),
            None => return Ok(()),
        }
        let doc = Document::from(d.as_str());
        let base_url = Url::parse(url.as_str())?;
        let title = doc.find(Name("title")).next().unwrap().text();
        downloader.args.datas.push(Data::new(&title, &url));
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
                    if !downloader.args.added_urls.contains(&new_url){
                        downloader.args.added_urls.insert(new_url.clone());
                        downloader.start(new_url,self, false);
                    }
                    
                }
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let mut download = Downloader::new(
        String::from(r"data/book"),
        String::from("https://doc.rust-lang.org/book/"),
        None,
        Manager{datas:Vec::new(), added_urls: HashSet::new()}
    );
    let url = String::from("https://doc.rust-lang.org/book/index.html");
    download.args.added_urls.insert(url.clone());
    download.start(url, &CrawlerData{}, false);
    download.wait().await?;
    println!("finish {}", download.args.datas.len());
    for item in download.args.datas.iter(){
        println!("{} {}", item.url, item.title);
    }
    Ok(())
}
