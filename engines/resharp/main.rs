use std::io::Write;

use {
    anyhow::Context,
    bstr::ByteSlice,
    lexopt::Arg,
    resharp::Regex,
};

fn main() -> anyhow::Result<()> {
    let mut p = lexopt::Parser::from_env();
    let (mut quiet, mut version) = (false, false);
    while let Some(arg) = p.next()? {
        match arg {
            Arg::Short('h') | Arg::Long("help") => {
                anyhow::bail!("main [--version | --quiet]")
            }
            Arg::Short('q') | Arg::Long("quiet") => {
                quiet = true;
            }
            Arg::Long("version") => {
                version = true;
            }
            _ => return Err(arg.unexpected().into()),
        }
    }
    if version {
        writeln!(std::io::stdout(), "{}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    let b = klv::Benchmark::read(std::io::stdin())
        .context("failed to read KLV data from <stdin>")?;
    let samples = match b.model.as_str() {
        "compile" => model_compile(&b)?,
        "count" => model_count(&b, &compile(&b)?)?,
        "count-spans" => model_count_spans(&b, &compile(&b)?)?,
        "grep" => model_grep(&b, &compile(&b)?)?,
        _ => anyhow::bail!("unrecognized benchmark model '{}'", b.model),
    };
    if !quiet {
        let mut stdout = std::io::stdout().lock();
        for s in samples.iter() {
            writeln!(stdout, "{},{}", s.duration.as_nanos(), s.count)?;
        }
    }
    Ok(())
}

fn model_compile(b: &klv::Benchmark) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run_and_count(
        b,
        |re: Regex| Ok(re.find_all(haystack)?.len()),
        || compile(b),
    )
}

fn model_count(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || Ok(re.find_all(haystack)?.len()))
}

fn model_count_spans(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || {
        Ok(re.find_all(haystack)?.iter().map(|m| m.end - m.start).sum())
    })
}

fn model_grep(
    b: &klv::Benchmark,
    re: &Regex,
) -> anyhow::Result<Vec<timer::Sample>> {
    let haystack = &*b.haystack;
    timer::run(b, || {
        let mut count = 0;
        for line in haystack.lines() {
            if re.is_match(line)? {
                count += 1;
            }
        }
        Ok(count)
    })
}

fn escape_resharp(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            out.push(c);
            if let Some(&next) = chars.peek() {
                out.push(chars.next().unwrap());
            }
        } else if c == '&' {
            out.push_str("\\&");
        } else if c == '~' {
            out.push_str("\\~");
        } else if c == '_' {
            out.push_str("\\_");
        } else {
            out.push(c);
        }
    }
    out
}

fn compile(b: &klv::Benchmark) -> anyhow::Result<Regex> {
    let pattern = b.regex.one()?;
    let pattern = escape_resharp(&pattern);
    let pattern = if b.regex.case_insensitive {
        format!("(?i)(?:{})", pattern)
    } else {
        pattern
    };
    let opts = resharp::EngineOptions::default()
        .unicode(b.regex.unicode);
    Ok(Regex::with_options(&pattern, opts)?)
}
