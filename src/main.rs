#![deny(rust_2018_idioms, unused, unused_import_braces, unused_qualifications, warnings)]

#[macro_use] extern crate clap;

use {
    std::{
        env,
        ffi::OsString,
        fs::{
            self,
            File
        },
        io::{
            self,
            prelude::*
        },
        net::{
            Ipv4Addr,
            UdpSocket
        },
        num::ParseIntError,
        os::unix::process::CommandExt as _,
        path::{
            Path,
            PathBuf
        },
        process::{
            Command,
            Stdio
        },
        str::FromStr as _,
        thread,
        time::Duration
    },
    chrono::prelude::*,
    clap::Arg,
    derive_more::From,
    gethostname::gethostname,
    itertools::Itertools as _,
    kuchiki::{
        NodeRef,
        traits::TendrilSink as _
    },
    reqwest::blocking::Client,
    serde_derive::Serialize,
    serde_json::json,
    crate::ib::{
        Ib,
        render_line
    }
};

mod ib;

#[derive(Debug, From)]
enum Error {
    Chrono(chrono::format::ParseError),
    Duration(time::OutOfRangeError),
    EmptyHostname,
    #[from(ignore)]
    Io(io::Error, Option<PathBuf>),
    Json(serde_json::Error),
    Parse(Option<&'static str>),
    ParseInt(ParseIntError),
    ParseTopRow(usize), //DEBUG
    Reqwest(reqwest::Error),
    TimeSet,
    UtfDecode(OsString)
}

impl From<()> for Error {
    fn from((): ()) -> Error {
        Error::Parse(None)
    }
}

trait IoResultExt {
    type T;

    fn at(self, path: impl AsRef<Path>) -> Self::T;
    fn at_unknown(self) -> Self::T;
}

impl IoResultExt for io::Error {
    type T = Error;

    fn at(self, path: impl AsRef<Path>) -> Error {
        Error::Io(self, Some(path.as_ref().to_owned()))
    }

    fn at_unknown(self) -> Error {
        Error::Io(self, None)
    }
}

impl<T, E: IoResultExt> IoResultExt for Result<T, E> {
    type T = Result<T, E::T>;

    fn at(self, path: impl AsRef<Path>) -> Result<T, E::T> {
        self.map_err(|e| e.at(path))
    }

    fn at_unknown(self) -> Result<T, E::T> {
        self.map_err(|e| e.at_unknown())
    }
}

#[derive(Serialize, PartialEq, Eq)]
struct Run {
    game: Ib<String>,
    category: Ib<String>,
    platform: Ib<String>,
    runners: Ib<String>,
    host: Ib<String>,
    setup_time: Ib<Duration>,
    start_time: Ib<DateTime<Utc>>,
    run_time: Ib<Duration>
}

impl Run {
    fn end_time(&self) -> Result<DateTime<Utc>, Error> {
        Ok(self.start_time.0 + chrono::Duration::from_std(self.run_time.0)?)
    }
}

fn get_schedule(client: &Client, event: usize) -> Result<Vec<Run>, Error> {
    let document = {
        let mut response = client.get(&format!("https://gamesdonequick.com/schedule/{}", event))
            .send()?
            .error_for_status()?;
        let mut response_content = String::default();
        response.read_to_string(&mut response_content).at_unknown()?;
        kuchiki::parse_html().one(response_content)
    };
    document
        .select_first("#runTable")?
        .as_node()
        .select_first("tbody")?
        .as_node()
        .children()
        .filter_map(NodeRef::into_element_ref)
        .map(|elt_ref| elt_ref.as_node().clone())
        .tuples()
        .map(|(top_row, bottom_row)| {
            let (start_time, game, runners, setup_time) = top_row.children().filter_map(NodeRef::into_element_ref).map(|elt_ref| elt_ref.text_contents().trim().to_string()).collect_tuple().ok_or(Error::ParseTopRow(top_row.children().count()))?;
            let (run_time, category_platform, host) = bottom_row.children().filter_map(NodeRef::into_element_ref).map(|elt_ref| elt_ref.text_contents().trim().to_string()).collect_tuple().ok_or(Error::Parse(Some("bottom row")))?;
            let (category, platform) = category_platform.splitn(2, " — ").map(String::from).collect_tuple().ok_or(Error::Parse(Some("category/platform")))?;
            Ok(Run {
                game: Ib(game),
                category: Ib(category),
                platform: Ib(platform),
                runners: Ib(runners),
                host: Ib(host),
                setup_time: Ib(parse_duration(setup_time)?),
                start_time: Ib(start_time.parse()?),
                run_time: Ib(parse_duration(run_time)?)
            })
        })
        .collect()
}

fn hostname() -> Result<String, Error> {
    gethostname()
        .into_string()?
        .split('.')
        .next()
        .ok_or(Error::EmptyHostname)
        .map(String::from)
}

fn parse_duration(duration_str: impl ToString) -> Result<Duration, Error> {
    let (hours, minutes, seconds) = duration_str.to_string().split(':').map(u64::from_str).collect_tuple().ok_or(Error::Parse(Some("parse_duration")))?;
    Ok(Duration::from_secs(hours? * 3600 + minutes? * 60 + seconds?))
}

fn setup_info_beamer() -> Result<(), Error> {
    write_loading_message("updating assets")?;
    fs::copy("/opt/git/github.com/fenhl/info-beamer-text/master/text.lua", "text.lua").at("/opt/git/github.com/fenhl/info-beamer-text/master/text.lua")?;
    fs::copy("fonts/dejavu/DejaVuSans.ttf", "dejavu_sans.ttf").at("fonts/dejavu/DejaVuSans.ttf")?;
    write_loading_message("starting info-beamer")?;
    Command::new("info-beamer")
        .arg(".")
        .env("INFOBEAMER_INFO_INTERVAL", "604800")
        .stdin(Stdio::null()) // to avoid terminating when pressing arrow keys
        .spawn()
        .at_unknown()?;
    write_loading_message("waiting to make sure socket will be available")?;
    thread::sleep(Duration::from_secs(10));
    Ok(())
}

fn write_loading_message(msg: &str) -> Result<(), Error> {
    serde_json::to_writer(File::create("data.json").at("data.json")?, &json!({
        "mode": "loading",
        "hostname": hostname()?,
        "loading": render_line(msg)
    }))?;
    Ok(())
}

fn main_inner() -> Result<(), Error> {
    write_loading_message("setting time")?;
    {
        let sock = UdpSocket::bind((Ipv4Addr::new(127, 0, 0, 1), 0)).at_unknown()?; // port 0 tells the OS to assign an arbitrary port
        let buf = format!("gdq/time/set:{}", Utc::now().timestamp()).into_bytes();
        if sock.send_to(&buf, (Ipv4Addr::new(127, 0, 0, 1), 4444)).at_unknown()? != buf.len() { return Err(Error::TimeSet); }
    }
    write_loading_message("determining current event")?;
    let client = Client::new();
    let event = 27; //TODO determine automatically
    write_loading_message("loading event schedule")?;
    let mut schedule = get_schedule(&client, event)?;
    serde_json::to_writer(File::create("data.json").at("data.json")?, &json!({
        "mode": "schedule",
        "schedule": schedule
    }))?;
    //TODO load bids from https://gamesdonequick.com/tracker/bids/<event_id>
    //TODO serialize again, with bids
    loop {
        thread::sleep(Duration::from_secs(60));
        if schedule.last().map_or(Ok::<_, Error>(true), |run| Ok(run.end_time()? <= Utc::now()))? { break; }
        let new_schedule = get_schedule(&client, event)?;
        //TODO load bids from https://gamesdonequick.com/tracker/bids/<event_id>
        if new_schedule != schedule /*TODO || new_bids != bids*/ { // only write data if it changes
            schedule = new_schedule;
            //TODO bids = new_bids;
            serde_json::to_writer(File::create("data.json").at("data.json")?, &json!({
                "mode": "schedule",
                "schedule": schedule
                //TODO bids
            }))?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Error> { //TODO Result<!, Error>
    let matches = app_from_crate!()
        .arg(Arg::with_name("exit")
            .short("x")
            .long("exit")
            .help("Exit info-beamer after the event has ended.")
        )
        .get_matches();
    let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).canonicalize().at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))?.join("assets");
    env::set_current_dir(&assets).at(assets)?;
    setup_info_beamer()?;
    main_inner()?;
    if !matches.is_present("exit") {
        loop { thread::park(); }
    } else {
        Err(Command::new("sudo").arg("--non-interactive").arg("killall").arg("info-beamer").exec().at_unknown()) //TODO get pid instead?
    }
}
