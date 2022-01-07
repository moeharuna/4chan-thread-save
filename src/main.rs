use clap::{App, Arg};
use rayon::prelude::*;
use scraper::{Html, Selector};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use ureq::Response;
use url::Url;
struct Enviroment {
    ignore_errors: bool,
    _parse_op_text: bool, //TODO:
    thread_url: Url,
    save_location: String,
}

fn get(path: &Url) -> Result<Response, ureq::Error> {
    Ok(ureq::get(&path.to_string()).call()?)
}

fn get_utf8(path: &Url) -> Result<String, ureq::Error> {
    Ok(get(path)?.into_string()?)
}

fn get_bytes(path: &Url) -> Result<Vec<u8>, ureq::Error> {
    let mut result: Vec<u8> = Vec::new();
    get(path)?.into_reader().read_to_end(&mut result).unwrap();
    Ok(result)
}

fn save_to_file(bytes: &[u8], path: &Path) -> std::io::Result<()> {
    if let Some(dir_to_file) = path.parent() {
        std::fs::create_dir_all(dir_to_file)?;
    }
    let mut file = File::create(path)?;
    file.write_all(bytes)
}

fn img_path_2_url(img_path: &str) -> Url {
    let url = "https:".to_string() + img_path;
    validate_url(&url)
}

fn images_in_thread_list(env: &Enviroment) -> Vec<Url> {
    let thread = get_utf8(&env.thread_url).unwrap();
    let thread = Html::parse_document(&thread);

    let image_div_selector = Selector::parse(r#"div[class="fileText"]"#).unwrap();
    let image_src_selector = Selector::parse("a[href]").unwrap();

    thread
        .select(&image_div_selector)
        .into_iter()
        .map(|div| {
            let div = Html::parse_fragment(&div.inner_html());
            let mut image_path = div.select(&image_src_selector);
            image_path
                .next()
                .unwrap()
                .value()
                .attr("href")
                .unwrap()
                .to_string()
        })
        .map(|path| img_path_2_url(&path))
        .collect()
}

fn thread_id_and_name(thread_url: &Url) -> (String, Option<String>) {
    let vec: Vec<String> = thread_url
        .path_segments()
        .unwrap()
        .map(|s| s.to_string())
        .collect();
    let (before_last, last) = (vec.len() - 2, vec.len() - 1);
    if vec.len() > 3 {
        (vec[before_last].clone(), Some(vec[last].clone())) //FIXME: rust not allows move out of vector, but it's clearly should be move not clone
    } else {
        (vec[last].clone(), None)
    }
}

fn url_to_file_path(img_url: &Url, env: &Enviroment) -> PathBuf {
    let image_id = img_url.path_segments().unwrap().last().unwrap();
    let (thread_id, thread_name) = thread_id_and_name(&env.thread_url);
    let mut result = PathBuf::from(&env.save_location);
    if let Some(thread_name) = thread_name {
        result.push(format!("{} - {}", thread_name, thread_id));
    } else {
        result.push(thread_id);
    }
    result.push(image_id);
    result
}

fn command_line_args() -> App<'static, 'static> {
    App::new("chan-image-save")
        .about("Saves all images from 4chan thread")
        .version("v0.1")
        .arg(
            Arg::with_name("thread-url")
                .required(true)
                .help("url of thread you want to save"),
        )
        .arg(
            Arg::with_name("save-location")
                .takes_value(true)
                .help("Sets place where thread will be saved.")
                .short("-s")
                .long("--save-location"),
        )
        .arg(
            Arg::with_name("ignore-errors")
                .short("-i")
                .long("--ignore-errors")
                .help("Trys to ignore images that can't be saved and conitnue with the others."),
        )
    //TODO:        .arg(Arg::with_name("parse-op-text").short("-p"))
}

fn get_enviroment() -> Enviroment {
    let matches = command_line_args().get_matches();
    let thread_url = matches.value_of("thread-url").unwrap().to_string();
    let save_location = match matches.value_of("save-location") {
        Some(val) => val,
        None => ".",
    }
    .to_string();
    let ignore_errors = matches.is_present("ignore-errors");
    let _parse_op_text = matches.is_present("parse-op-text");

    let thread_url = validate_url(&thread_url);

    Enviroment {
        thread_url,
        save_location,
        ignore_errors,
        _parse_op_text,
    }
}

fn validate_url(thread_url: &str) -> Url {
    let url = Url::parse(thread_url).unwrap();
    let scheme = url.scheme();
    assert!(scheme == "http" || scheme == "https");
    url
}

fn non_critical_error(env: &Enviroment, text: String) {
    if env.ignore_errors {
        eprintln!("{}", text);
    } else {
        panic!("{}", text)
    }
}

fn main() {
    //TODO:
    //Option for parsing OP text into .txt
    //Better error handling
    //Make structs instead of strings and urls?
    //other chans support?
    let env = get_enviroment();
    let images_urls = images_in_thread_list(&env);
    images_urls.par_iter().for_each(|image_url| {
        let path = url_to_file_path(&image_url, &env);
        match get_bytes(&image_url) {
            Ok(bytes) => match save_to_file(&bytes, &path) {
                Ok(_) => println!("{}", path.to_str().unwrap()),

                Err(e) => non_critical_error(
                    &env,
                    format!(
                        "Failed to save a file({}), reason: {}",
                        path.to_str().unwrap(),
                        e
                    ),
                ),
            },

            Err(e) => {
                non_critical_error(
                    &env,
                    format!("Failed to get a image({}), reason: {}", &image_url, e),
                );
            }
        }
    });
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn thread_id_and_name_test() {
        let url = Url::parse("https://chan.org/b/thread/666/name");
        let (thread_id, thread_name) = thread_id_and_name(&url.unwrap());
        assert_eq!(thread_id, "666");
        assert_eq!(thread_name, Some("name".to_string()));
    }
    #[test]
    fn thread_id_no_name_test() {
        let url = Url::parse("https://chan.org/b/thread/666");
        let (thread_id, thread_name) = thread_id_and_name(&url.unwrap());
        assert_eq!(thread_id, "666");
        assert_eq!(thread_name, None);
    }
    #[test]
    fn url_to_file_path_test() {
        let thread_url = Url::parse("https://chan.org/b/thread/666/name").unwrap();
        let env = Enviroment {
            ignore_errors: false,
            _parse_op_text: false,
            thread_url,
            save_location: "/pictures".to_string(),
        };
        let image_url = Url::parse("https://i.4cdn.org/b/1.jpg").unwrap();
        assert_eq!(
            url_to_file_path(&image_url, &env).to_str(),
            Some("/pictures/name - 666/1.jpg")
        )
    }
}
