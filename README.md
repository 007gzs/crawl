## Crawl
Rust crawl

### demo
```
use std::error::Error as StdError;
use crawl::downloader::{Downloader, Crawler};
#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let mut download = Downloader::new(
        String::from(r"data/book"),
        String::from("https://doc.rust-lang.org/book/"),
        None,
        Manager{datas:Vec::new(), added_urls: HashSet::new()}
    );
    let url = String::from("https://doc.rust-lang.org/book/index.html");
    download.crawl(url, &CrawlerData{}, false).await?;
    Ok(())
}

```