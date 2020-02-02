use html5ever::tendril::TendrilSink;
use markup5ever_rcdom as rcdom;
use std::*;
use unicode_width::UnicodeWidthStr;

struct Paragraph {
    text: String,
    anchor_len2: usize,
    normal_len2: usize,
}

fn flatten_dom_inner(
    node: rcdom::Handle,
    is_anchored: bool,
    buf: &mut Paragraph,
    dst: &mut Vec<Paragraph>,
) {
    // ref. <https://www.w3schools.com/cssref/css_default_values.asp>.
    thread_local!(static IGNORE_ELEMENTS: collections::HashSet<&'static str> = [
        "area", "base", "datalist", "head", "link", "meta", "noscript", "param", "script", "style",
        "title",
    ].iter().cloned().collect());
    thread_local!(static BLOCK_ELEMENTS: collections::HashSet<&'static str> = [
        "address", "article", "aside", "blockquote", "body", "caption", "colgroup", "dd",
        "details", "div", "dl", "dt", "fieldset", "figcaption", "figure", "footer", "form", "h1",
        "h2", "h3", "h4", "h5", "h6", "header", "hr", "html", "iframe", "legend", "li", "menu",
        "nav", "ol", "p", "pre", "section", "summary", "table", "tbody", "tfoot", "thead", "tr",
        "ul",
    ].iter().cloned().collect());

    match node.data {
        rcdom::NodeData::Doctype { .. } => (),
        rcdom::NodeData::Comment { .. } => (),
        rcdom::NodeData::Document => {
            for child in node.children.borrow().iter() {
                flatten_dom_inner(child.clone(), is_anchored, buf, dst);
            }
        }
        rcdom::NodeData::Text { ref contents } => {
            thread_local!(static RE_SPACES: regex::Regex = regex::Regex::new(r"[ \r\t\n]+").unwrap());
            let text = contents.borrow();
            let text = RE_SPACES.with(|re| re.replace_all(&text, " "));
            let len2 = if buf.text.ends_with(" ") && text.starts_with(" ") {
                buf.text.push_str(&text[1..]);
                2 * text.width() - 1
            } else {
                buf.text.push_str(&text);
                2 * text.width()
            };
            if is_anchored {
                buf.anchor_len2 += len2;
            } else {
                buf.normal_len2 += len2;
            }
        }
        rcdom::NodeData::Element { ref name, .. } => {
            let name: &str = &name.local.to_ascii_lowercase();
            if IGNORE_ELEMENTS.with(|e| e.contains(name)) {
            } else if BLOCK_ELEMENTS.with(|e| e.contains(name)) {
                let text = buf.text.trim();
                if text.len() > 0 {
                    dst.push(Paragraph {
                        text: text.to_string(),
                        ..*buf
                    });
                }
                buf.text.clear();
                buf.normal_len2 = 0;
                buf.anchor_len2 = 0;

                for child in node.children.borrow().iter() {
                    flatten_dom_inner(child.clone(), is_anchored, buf, dst);
                }

                let text = buf.text.trim();
                if text.len() > 0 {
                    dst.push(Paragraph {
                        text: text.to_string(),
                        ..*buf
                    });
                }
                buf.text.clear();
                buf.normal_len2 = 0;
                buf.anchor_len2 = 0;
            } else {
                for child in node.children.borrow().iter() {
                    flatten_dom_inner(child.clone(), is_anchored || name == "a", buf, dst);
                }
            }
        }
        rcdom::NodeData::ProcessingInstruction { .. } => unreachable!(),
    }
}

fn flatten_dom(node: rcdom::Handle) -> Vec<Paragraph> {
    let mut buf = Paragraph {
        text: String::new(),
        normal_len2: 0,
        anchor_len2: 0,
    };
    let mut dst = Vec::new();
    flatten_dom_inner(node, false, &mut buf, &mut dst);
    let text = buf.text.trim();
    if text.len() > 0 {
        dst.push(Paragraph {
            text: text.to_string(),
            ..buf
        });
    }
    dst
}

fn remove_numbers(text: &str) -> String {
    thread_local!(static RE_NUMBER: regex::Regex = regex::Regex::new(r"[-+]?[0-9][0-9,]*(\.[0-9]+)?").unwrap());
    let text = RE_NUMBER.with(|re| re.replace_all(text, "0"));
    text.to_string()
}

fn count_duplicates(src: &[Paragraph]) -> Vec<usize> {
    let mut counter: collections::HashMap<&String, usize> = collections::HashMap::new();
    let keys: Vec<String> = src.iter().map(|e| remove_numbers(&e.text)).collect();
    for i in 0..src.len() {
        *counter.entry(&keys[i]).or_insert(0) += 1;
    }
    keys.iter().map(|k| *counter.get(k).unwrap()).collect()
}

fn mean_and_deviation(xs: &[f64]) -> (f64, f64) {
    let n = xs.len();
    let m = xs.iter().sum::<f64>() / n as f64;
    let s = xs.iter().map(|e| (e - m) * (e - m)).sum::<f64>() / (n - 1) as f64;
    (m, f64::sqrt(s))
}

fn fill_up_hole(xs: &mut [f64]) {
    for i in 1..xs.len() - 1 {
        xs[i] = f64::max(xs[i], f64::min(xs[i - 1], xs[i + 1]));
    }
}

fn calc_scores(src: &[Paragraph]) -> Vec<f64> {
    let counts = count_duplicates(src);
    let lls: Vec<f64> = src
        .iter()
        .map(|e| f64::ln((e.normal_len2 + e.anchor_len2) as f64))
        .chain([0.0, 6.0].iter().copied())
        .collect();
    let nrs: Vec<f64> = src
        .iter()
        .map(|e| e.normal_len2 as f64 / (e.normal_len2 + e.anchor_len2) as f64)
        .chain([0.0, 1.0].iter().copied())
        .collect();
    let (lls_m, lls_d) = mean_and_deviation(&lls);
    let (nrs_m, nrs_d) = mean_and_deviation(&nrs);
    let mut dst: Vec<f64> = (0..src.len())
        .map(|i| {
            let llr = (lls[i] - lls_m) / lls_d;
            let nrr = (nrs[i] - nrs_m) / nrs_d;
            llr + nrr - f64::log2(counts[i] as f64) + 1.0
        })
        .collect();
    fill_up_hole(&mut dst);
    dst
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let args: Vec<String> = env::args().collect();
    assert!(args.len() == 2);

    let client = reqwest::blocking::Client::new();
    let mut buf = client.get(&args[1]).send()?;

    let dom = html5ever::parse_document(rcdom::RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut buf)?
        .document;
    let seq = flatten_dom(dom);
    let scores = calc_scores(&seq);

    for i in 0..seq.len() {
        println!("{:+.2} {}", scores[i], seq[i].text);
    }

    Ok(())
}
