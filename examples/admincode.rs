use std::error::Error as StdError;
use std::fs::File;  
use std::io::Write;
use std::fmt::{self, Display, Formatter};
use async_trait::async_trait;
use crawl::downloader::{Downloader, GetProxy, Crawler};
use select::document::Document;
use select::node::Node;
use select::predicate::{Name, Class, Predicate};
use url::Url;
use bytes::Bytes;
use encoding_rs::{UTF_8, GB18030};

#[derive(Clone, PartialEq)]
enum CityType{
    Country,
    Province,
    City,
    County,
    Town,
    Village
}
impl Display for CityType {  
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {  
        match self {  
            CityType::Country => write!(f, "country"),  
            CityType::Province => write!(f, "province"),  
            CityType::City => write!(f, "city"),  
            CityType::County => write!(f, "county"),  
            CityType::Town => write!(f, "town"),  
            CityType::Village => write!(f, "village"),  
        }  
    }  
}
struct AdminCode{
    year: u16,
    code: String,
    parent_code: String,
    short_code: String,
    name: String,
    short_name: String,
    city_type: CityType,
    town_type_code: String
}
fn rstrip<'a>(name: &'a str, end: &'a str) -> &'a str{
    if name.ends_with(end) && name.chars().count() - end.chars().count() >= 2{
        &name[0..(name.len() - end.len())]
    }else {
        name
    }
}
const MINGZU:[&str; 55] =  [
    "蒙古族","回族","藏族","维吾尔族","苗族","彝族","壮族","布依族","朝鲜族","满族","侗族","瑶族","白族",
    "土家族","哈尼族","哈萨克族","傣族","黎族","傈僳族","佤族","畲族","高山族","拉祜族","水族","东乡族",
    "纳西族","景颇族","柯尔克孜族","土族","达斡尔族","仫佬族","羌族","布朗族","撒拉族","毛南族","仡佬族",
    "锡伯族","阿昌族","普米族","塔吉克族","怒族","乌孜别克族","俄罗斯族","鄂温克族","德昂族","保安族",
    "裕固族","京族","塔塔尔族","独龙族","鄂伦春族","赫哲族","门巴族","珞巴族","基诺族"
];

fn name_to_short_name<'a>(name: &'a str, parent_short_name:&'a str, city_type:&CityType) -> &'a str{
    if name == "中华人名共和国"{
        return "中国";
    }
    if vec!("市辖区", "省直辖县级行政区划", "自治区直辖县级行政区划", "县").contains(&name){
        return parent_short_name;
    }
    let mut short_name = name;
    short_name = rstrip(short_name, "自治县");
    short_name = rstrip(short_name, "自治区");
    short_name = rstrip(short_name, "自治旗");
    short_name = rstrip(short_name, "自治州");
    short_name = rstrip(short_name, "办事处");
    short_name = rstrip(short_name, "村民委员会");
    short_name = rstrip(short_name, "村委会");
    short_name = rstrip(short_name, "居民委员会");
    short_name = rstrip(short_name, "居委会");
    short_name = rstrip(short_name, "街道");
    short_name = rstrip(short_name, "社区");
    short_name = rstrip(short_name, "地区");
    if vec!(CityType::Province, CityType::City, CityType::Country).contains(city_type){
        short_name = rstrip(short_name, "省");
        short_name = rstrip(short_name, "市");
        if !short_name.ends_with("新区") && !short_name.ends_with("矿区"){
            short_name = rstrip(short_name, "区");
        }
        short_name = rstrip(short_name, "县");
        short_name = rstrip(short_name, "旗");
        short_name = rstrip(short_name, "盟");
    }
    short_name = rstrip(short_name, "镇");
    if !short_name.ends_with("新村"){
        short_name = rstrip(short_name, "村");
    }
    short_name = rstrip(short_name, "乡");
    while short_name.chars().count() > 2 && short_name.ends_with("族"){
        let len = short_name.chars().count();
        for mz in MINGZU.iter(){
            short_name = rstrip(short_name, mz);
        }
        if len == short_name.chars().count(){
            break;
        }
    }
    
    while short_name.chars().count() > 2{
        let len = short_name.chars().count();
        for mz in MINGZU.iter(){
            short_name = rstrip(short_name, &mz[0..(mz.len() - mz.chars().last().unwrap().len_utf8())]);
        }
        if len == short_name.chars().count(){
            break;
        }
    }
    short_name
}
impl AdminCode{
    fn new(
        year: u16,
        code: &str,
        parent_code: &str,
        short_code: &str,
        name: &str,
        short_name: &str,
        city_type: CityType,
        town_type_code: &str
    ) -> AdminCode{
        AdminCode{
            year: year,
            parent_code: parent_code.to_string(),
            code: code.to_string(),
            short_code: short_code.to_string(),
            name: name.to_string(),
            short_name: short_name.to_string(),
            city_type: city_type,
            town_type_code: town_type_code.to_string()
        }
    }
    fn create(year: u16, code: &str, parent_code:&str, name: &str, parent_short_name:&str, city_type: CityType, town_type_code:&str) -> AdminCode{
        let short_code = if code.len() >= 6{
            &code[0..6]
        }else{
            code
        };
        let short_name = name_to_short_name(name, parent_short_name, &city_type);
        AdminCode::new(year, code, parent_code, short_code, name, short_name, city_type, town_type_code)

    }
    fn china(year: u16) -> AdminCode{
        AdminCode::create(year, "000000000000","", "中华人名共和国", "", CityType::Country, "")
    }

    
}
struct GetCnProxy{
    proxy_url: String
}
#[async_trait]
impl GetProxy for GetCnProxy{
    async fn get_proxy(&self) -> Result<Option<reqwest::Proxy>, Box<dyn StdError>>{
        let body = reqwest::get(self.proxy_url.clone()).await?.text().await?;
        let proxy = reqwest::Proxy::http(format!("http://{}", body))?;
        Ok(Some(proxy))
    }
}
struct CrawlerBase{
    year: u16,
    parent_short_name: String,
    parent_code: String,
}
struct CrawlerRoot{
    base: CrawlerBase,
}
struct CrawlerData{
    base: CrawlerBase,
}

fn get_text_href(node:Node)-> (String, Option<&str>){
    match node.find(Name("a")).next(){
        Some(a)=>(a.text(), a.attr("href")),
        None=>(node.text(), None)
    }
}
impl CrawlerBase {
    fn new(year:u16,parent_code: &str, parent_short_name:& str) -> CrawlerBase{
        CrawlerBase{year: year, parent_code: parent_code.to_string(), parent_short_name:parent_short_name.to_string()}
    }
    async fn parse_trs(&self, downloader:&mut Downloader<Vec<AdminCode>>, base_url:&Url, doc:&Document, class_name:&str, city_type:CityType)-> Result<(), Box<dyn StdError>>{

        for node in doc.find(Class(class_name)){
            let mut tds = node.find(Name("td"));
            let (code, href1) = match tds.next(){
                None=>(String::new(), None),
                Some(td)=>get_text_href(td)
            };
            let (text, href2) = match tds.next(){
                None=>(String::new(), None),
                Some(td)=>get_text_href(td)
            };
            let href = match href1{
                Some(h)=>Some(h),
                None=>href2
            };
            let town_type_code;
            let name;
            if city_type == CityType::Village{
                match tds.next(){
                    None=>{
                        town_type_code = String::new();
                        name = text;
                    },
                    Some(td)=>{
                        town_type_code = text;
                        name = td.text();
                    }
                }
            }else{
                town_type_code = String::new();
                name = text;
            }
            let admin_code = AdminCode::create(self.year, &code, &self.parent_code, &name, &self.parent_short_name,  city_type.clone(), &town_type_code);
            let short_name = admin_code.short_name.clone();
            downloader.args.push(admin_code);
            match href{
                None=>{},
                Some(h)=>{
                    let new_url = base_url.join(&h)?;
                    downloader.crawl(new_url.to_string(), &CrawlerData{base:CrawlerBase::new(self.year, &code, &short_name)}, false).await?;
                }
            }
        }
        Ok(())
    }
}
fn decode_bytes(data:&Bytes) -> Option<String>{
    let (text, _, _) = GB18030.decode(data.as_ref());
    if text.contains("代码"){
        return Some(text.into_owned());
    }
    
    let (text, _, _) = UTF_8.decode(data.as_ref());
    if text.contains("代码"){
        return Some(text.into_owned());
    }
    return None;
    
}
#[async_trait(?Send)]
impl Crawler<Vec<AdminCode>> for CrawlerData{
    async fn parse(&self, downloader:&mut Downloader<Vec<AdminCode>>, url:String, data:Option<Bytes>) -> Result<(), Box<dyn StdError>> {
        let d;
        match data {
            Some(v) => match decode_bytes(&v){
                Some(text) => d = text,
                None => {
                    downloader.crawl(url, self, true).await?;
                    return Ok(());
                }
            },
            None => return Ok(()),
        }
        let doc = Document::from(d.as_str());
        let base_url = Url::parse(url.as_str())?;
        self.base.parse_trs(downloader, &base_url, &doc, "citytr", CityType::City).await?;
        self.base.parse_trs(downloader, &base_url, &doc, "countytr", CityType::County).await?;
        self.base.parse_trs(downloader, &base_url, &doc, "towntr", CityType::Town).await?;
        self.base.parse_trs(downloader, &base_url, &doc, "villagetr", CityType::Village).await?;
        Ok(())
    }
}
#[async_trait(?Send)]
impl Crawler<Vec<AdminCode>> for CrawlerRoot{
    async fn parse(&self, downloader:&mut Downloader<Vec<AdminCode>>, url:String, data:Option<Bytes>) -> Result<(), Box<dyn StdError>> {
        let d;
        match data {
            Some(v) => match decode_bytes(&v){
                Some(text) => d = text,
                None => {
                    downloader.crawl(url, self, true).await?;
                    return Ok(());
                }
            },
            None => return Ok(()),
        }

        let doc = Document::from(d.as_str());
        let base_url = Url::parse(url.as_str())?;
        for node in doc.find(Class("provincetr").descendant(Name("a"))){
            let href = node.attr("href").unwrap();
            let name = node.text();
            let code = match href.split_once("."){
                Some((c, _)) => format!("{}0000000000", c).to_string(),
                None => String::new(),
            };
            let admin_code = AdminCode::create(self.base.year, &code, &self.base.parent_code, &name, &self.base.parent_short_name, CityType::Province, "");
            let short_name = admin_code.short_name.clone();
            downloader.args.push(admin_code);
            let new_url = base_url.join(href)?;

            downloader.crawl(new_url.to_string(), &CrawlerData{base:CrawlerBase::new(self.base.year, &code, &short_name)}, false).await?;
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn StdError>> {
    let mut download = Downloader::new(
        String::from(r"data"),
        String::from("https://www.stats.gov.cn/sj/tjbz/tjyqhdmhcxhfdm/"),
        None,
        Vec::new()
    );
    //let mut handles = Vec::new();
    for year in 2009..=2023{
        let china = AdminCode::china(year);
        let code = china.code.clone();
        let short_name = china.short_name.clone();
        download.args.push(china);
        let url = format!("https://www.stats.gov.cn/sj/tjbz/tjyqhdmhcxhfdm/{}/index.html", year);
        //handles.push(
            download.crawl(url, &CrawlerRoot{base: CrawlerBase::new(year, &code, &short_name)}, false)
            .await?;
        //);
    }
    //futures::future::join_all(handles).await;
    println!("finish {}", download.args.len());
    let mut file = File::create("admin_code.csv")?;
    write!(file, "year,code,parent_code,short_code,name,short_name,city_type,town_type_code\n")?;
    for item in download.args.iter(){
        write!(file, "{},{},{},{},{},{},{},{}\n", item.year, item.code, item.parent_code, item.short_code, item.name, item.short_name, item.city_type, item.town_type_code)?;
    }
    Ok(())
}
