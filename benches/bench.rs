#![feature(test)]

extern crate pinyin;
extern crate test;

#[bench]
fn bench_weicheng(b: &mut test::Bencher) {
    let text = include_str!("../data/weicheng.txt");
    
    b.bytes = text.len() as _;
    b.iter(|| {
        let args = pinyin::Args::new();
        pinyin::pinyin(text, &args);
    })
}