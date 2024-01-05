use std::fs::File;  
use std::io::{Write, Read}; 
use bytes::Bytes;
use std::fs;
use std::error::Error as StdError;  
use std::path::Path;
use std::time::Duration;
use async_trait::async_trait;
use reqwest;
use tokio::time::sleep;

#[async_trait]
pub trait GetProxy {
    async fn get_proxy(&self) -> Result<Option<reqwest::Proxy>, Box<dyn StdError>>;
}
pub struct Downloader<T>
{
    pub root_path: String,
    pub base_url: String,
    pub get_proxy: Option<Box<dyn GetProxy>>,
    pub args:T,
}

#[async_trait(?Send)]
pub trait Crawler<T> {
    async fn parse(&self, downloader:&mut Downloader<T>, url:String, data:Option<Bytes>) -> Result<(), Box<dyn StdError>>;
}

impl<T>  Downloader<T>
{
    pub fn new(root_path: String, base_url: String, get_proxy:Option<Box<dyn GetProxy>>, args:T) -> Downloader<T/*, ProxyCallback, ProxyFut */>{
        Downloader{root_path: root_path, base_url: base_url, get_proxy: get_proxy, args:args}
    }
    async fn get_proxy(&self) -> Result<Option<reqwest::Proxy>, Box<dyn StdError>>{
        match &self.get_proxy{
            None => Ok(None),
            Some(t) => {
                Ok(t.get_proxy().await?)
            }
        }
    }
    async fn connect_real(&self, url:String, proxy:Option<reqwest::Proxy>) -> Result<Bytes, reqwest::Error>{
        let mut builder = reqwest::Client::builder();
        match proxy {
            Some(p)=>{
                builder = builder.proxy(p);
            },
            None=>{}
        }
        let res = builder.build()?.get(url).send().await?;
        let body = res.bytes().await?;
        Ok(body)
    }
    
    async fn download(&self, url:String, force:bool) -> Result<Option<Bytes>, Box<dyn StdError>>{
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

        for retry in 0..5 {
            let proxy = match self.get_proxy().await {
                Ok(p) => p,
                Err(_) => None
            };
            match self.connect_real(url.clone(), proxy).await{
                Ok(body) => {
                    let mut file = File::create(path)?;
                    for chunk in body.chunks(4096){
                        file.write(chunk)?;
                    }
                    return Ok(Some(body));
                },
                Err(_) => {println!("error {} {}", retry, url); sleep(Duration::from_secs(1)).await;}
            };
        }
        Ok(None)
    }
    pub async fn crawl(&mut self, url:String, callback: &dyn Crawler<T>, force:bool)-> Result<(), Box<dyn StdError>>
    {
        let data = self.download(url.clone(), force).await?;
        callback.parse(self, url, data).await?;
        Ok(())
    }

}