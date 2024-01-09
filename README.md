## Crawl
Rust crawl

### demo
```

fn main() -> anyhow::Result<()> {
    let download = Arc::new(Downloader::new(
        String::from(r"data/book1"),
        String::from("https://doc.rust-lang.org/book/")
    ));
    let url = String::from("https://doc.rust-lang.org/book/index.html");
    let manager = Arc::new(Mutex::new(Manager{datas:Vec::new(), added_urls:HashSet::from([url.clone()])}));
    download.start_url(url, false, Arc::new(Flag::Page))?;
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

```