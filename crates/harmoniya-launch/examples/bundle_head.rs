//! Read a published bundle's head — one range request off the front of the
//! file — and print the settings form a player would see, in both UI
//! languages, plus the vars and features its defaults resolve to.
//!
//!     cargo run -p harmoniya-launch --example bundle_head -- <bundle url>

use harmoniya_launch::options::{self, Head, ModpackOptions, OptionDef};

fn print_options(options: &[OptionDef], head: &Head, lang: &str, depth: usize) {
    let tr = |text| options::localize(text, lang, head.locale.as_deref());
    for option in options {
        let label = option.label();
        let pad = "  ".repeat(depth + 1);
        let subtitle = label.subtitle.as_ref().map(|s| format!(" — {}", tr(s))).unwrap_or_default();
        let kind = match option {
            OptionDef::Slider { min, max, step, default, unit, .. } => {
                let unit = unit.as_ref().map(|u| format!(" {}", tr(u))).unwrap_or_default();
                format!("slider {min}..{max} step {step}, default {default}{unit}")
            }
            OptionDef::Select { choices, default, .. } => {
                let choices: Vec<_> = choices.iter().map(|c| format!("{}={}", c.value, tr(&c.label))).collect();
                format!("select [{}], default {default}", choices.join(", "))
            }
            OptionDef::Text { default, .. } => format!("text, default {default:?}"),
            OptionDef::File { .. } => "file".to_owned(),
            OptionDef::Directory { .. } => "directory".to_owned(),
            OptionDef::Feature { default, .. } => format!("feature, default {default}"),
        };
        println!("{pad}{} `{}`: {kind}{subtitle}", tr(&label.title), option.name());
        if let OptionDef::Feature { options, .. } = option {
            print_options(options, head, lang, depth + 1);
        }
    }
}

fn main() {
    let url = std::env::args().nth(1).expect("usage: bundle_head <bundle url>");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let head = match runtime.block_on(options::read_head(&url)) {
        Ok(head) => head,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    println!("format {}, locale {:?}, {} option(s)", head.format, head.locale, head.options.as_slice().len());
    for lang in ["uk", "en"] {
        println!("[{lang}]");
        print_options(head.options.as_slice(), &head, lang, 0);
    }
    let (vars, features) = options::resolve(&head.options, &ModpackOptions::default());
    println!("defaults resolve to vars {vars:?}, features {features:?}");
}
