extern crate phf_codegen;

use std::env;
use std::fs::File;
use std::process::Command;
use std::collections::HashSet;
use std::path::{ Path, PathBuf, };
use std::io::{ BufWriter, Write, };


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub is_nightly: bool,
    pub major: usize,
    pub minor: usize,
    pub patch: usize,
}

impl Default for Version {
    fn default() -> Self {
        get_rustc_version().expect("oops ...")
    }
}

fn get_rustc() -> PathBuf {
    env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()).into()
}

fn get_rustc_version() -> Option<Version> {
    let rustc = get_rustc();
    let output = Command::new(&rustc)
        .args(&["--version", "--verbose"])
        .output()
        .ok()?;

    if output.stdout.len() == 0 {
        return None;
    }

    let s = String::from_utf8(output.stdout).ok()?;
    let version = s.lines()
        .find(|line| line.starts_with("release: "))
        .map(|release_line| &release_line["release: ".len()..])?;

    if version.len() == 0 {
        return None
    }

    let tmp = version.split("-").collect::<Vec<&str>>();
    let mut tmp = tmp[0].split('.');

    let is_nightly = if version.contains("nightly") { true } else { false };
    let major = tmp.next()?.parse::<usize>().ok()?;
    let minor = tmp.next()?.parse::<usize>().ok()?;
    let patch = tmp.next()?.parse::<usize>().ok()?;

    Some(Version { is_nightly, major, minor, patch, })
}


fn build_data() -> (Vec<(char, &'static str)>, Vec<(char, String)>) {
    let dict = include_str!("./pinyin-data/pinyin.txt");

    // 带声调字符 PHONETIC_SYMBOL_MAP
    let mut phonetic_symbol_map = vec![
        ('ā', "a1"),
        ('á', "a2"),
        ('ǎ', "a3"),
        ('à', "a4"),
        ('ē', "e1"),
        ('é', "e2"),
        ('ě', "e3"),
        ('è', "e4"),
        ('ō', "o1"),
        ('ó', "o2"),
        ('ǒ', "o3"),
        ('ò', "o4"),
        ('ī', "i1"),
        ('í', "i2"),
        ('ǐ', "i3"),
        ('ì', "i4"),
        ('ū', "u1"),
        ('ú', "u2"),
        ('ǔ', "u3"),
        ('ù', "u4"),
        ('ü', "v0"),
        ('ǘ', "v2"),
        ('ǚ', "v3"),
        ('ǜ', "v4"),
        ('ń', "n2"),
        ('ň', "n3"),
        ('', "m2"),
    ];
    let phonetic_symbol_set = phonetic_symbol_map
        .iter()
        .map(|item| item.0)
        .collect::<HashSet<char>>();
    assert_eq!(phonetic_symbol_map.len(), phonetic_symbol_set.len());

    // 拼音库    PINYIN_MAP
    let mut pinyin_map: Vec<(char, String)> = Vec::new();
    let mut pinyin_set: HashSet<char> = HashSet::new();

    for line in dict.lines() {
        let line = line.trim();

        if !line.starts_with('#') {
            let kv = line.split(':').collect::<Vec<&str>>();
            assert_eq!(kv.len() >= 2, true);

            let k = kv[0].trim();
            let v = kv[1].trim();

            let c: char = {
                assert_eq!(k.starts_with("U+"), true);
                let tmp = k.split("U+").collect::<Vec<&str>>();
                assert_eq!(tmp.len(), 2);

                let code_point = u32::from_str_radix(tmp[1], 16).unwrap();
                ::std::char::from_u32(code_point).unwrap()
            };

            let pinyin_list: Vec<String> = {
                let tmp = v.split('#').collect::<Vec<&str>>();
                assert_eq!(tmp.len(), 2);
                let pinyin_list = tmp[0]
                    .split(',')
                    .map(|item| item.trim().to_string())
                    .collect::<Vec<String>>();

                let comment = tmp[1].trim().chars().collect::<Vec<char>>()[0];
                assert_eq!(comment, c);

                pinyin_list
            };

            if pinyin_set.insert(c) {
                let item = (c, pinyin_list.join(","));
                pinyin_map.push(item);
            } else {
                println!("[DEBUG] 重复的数据: {}", line);
            }
        }
    }
    assert_eq!(pinyin_map.len(), pinyin_set.len());

    // NOTE: 使用稳定版的 sort
    phonetic_symbol_map.sort_by_key(|&(k, _)| k);
    pinyin_map.sort_by_key(|&(k, _)| k);

    (phonetic_symbol_map, pinyin_map)
}

fn codegen(phonetic_symbol_data: Vec<(char, &'static str)>, pinyin_data: Vec<(char, String)>) {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("codegen.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    let template = format!("
pub static PHONETIC_SYMBOL_DATA: [(char, &'static str); {}] = {:?};
pub static PINYIN_DATA: [(char, &'static str); {}] = {:?};",
        phonetic_symbol_data.len(),
        phonetic_symbol_data,
        pinyin_data.len(),
        pinyin_data,
    );
    write!(&mut file, "{}\n\n", template).unwrap();

    if env::var_os("CARGO_FEATURE_HASHMAP").is_none() {
        return ();
    }

    // PHF Code Gen
    write!(&mut file, "pub static PHONETIC_SYMBOL_MAP: phf::Map<char, &'static str> =\n").unwrap();
    let mut map = phf_codegen::Map::new();
    for (c, p) in phonetic_symbol_data {
        map.entry(c, &format!("\"{}\"", p));
    }
    map.build(&mut file).unwrap();
    write!(&mut file, ";\n\n").unwrap();

    write!(&mut file, "pub static PINYIN_MAP: phf::Map<char, &'static str> =\n").unwrap();
    let mut map = phf_codegen::Map::new();
    for (c, p) in pinyin_data {
        map.entry(c, &format!("\"{}\"", p));
    }
    map.build(&mut file).unwrap();
    write!(&mut file, ";\n\n").unwrap();
}


fn main() {
    let version = Version::default();

    if version.is_nightly {
        println!("cargo:rustc-cfg=feature=\"nightly\"");
    }

    let (phonetic_symbol_data, pinyin_data) = build_data();
    codegen(phonetic_symbol_data, pinyin_data);
}
