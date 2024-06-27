use clap::{Args, Parser};

#[derive(Args)]
#[group(required = false, multiple = true)]
struct Opts {
    #[arg(short = 't', long, name = "TOKEN")]
    token: Option<String>,

    #[arg(short = 'n', long, name = "NUM_LIMIT")]
    num_limit: Option<usize>,

    #[arg(short = 'd', long, name = "DIR", help = "Save it to `$DIR` or `.` ")]
    dir: Option<String>,
    #[arg(short = 'm', long, name = "MIRROR", help = "Not yet applied")]
    mirror: Option<String>,
    #[arg(short = 'p', long, name = "PROXY", help = "Not yet applied")]
    proxy: Option<String>,
}

#[derive(Parser)]
struct Cli {
    url: String,

    #[command(flatten)]
    opt: Opts,
}

fn main(){
    let start_time = std::time::Instant::now();
    let cli = Cli::parse();

    ld_::interface(
        &cli.url,
        cli.opt.token.as_deref(),
        cli.opt.dir.as_deref(),
        cli.opt.mirror.as_deref(),
        cli.opt.num_limit
    );

    println!("Processing time: {:?}", start_time.elapsed());
}
