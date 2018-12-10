extern crate clap;
extern crate float_cmp;
extern crate kstat;
extern crate zonename;

use clap::{Arg, App};
use float_cmp::ApproxEq;
use kstat::kstat_named::KstatNamedData;
use kstat::{KstatData, KstatReader};
use std::collections::HashMap;
use std::io::Write;
use std::{thread, time};

// TODO clean up these macros
macro_rules! header_fmt {
    () => ("{:>5} {:>5} {:>5} {:>5} {:>5} {:>5} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {}");
}

macro_rules! output_fmt {
    () => ("{:>5.1} {:>5.1} {:>5.1} {:>5.1} {:>5.1} {:>5.1} {:>6.1} {:>6.1} {:>6.1} {:>6.1} {:>6.1} {:>6.1} {} ({})");
}

macro_rules! write_header(
    ($($arg:tt)*) => { {
        let r = writeln!(&mut ::std::io::stderr(), header_fmt!(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);

macro_rules! write_output(
    ($($arg:tt)*) => { {
        let r = writeln!(&mut ::std::io::stderr(), output_fmt!(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);

fn print_header(hide: bool) {
    if hide {
        return;
    }
    write_header!("r/s", "w/s", "kr/s", "kw/s", "ractv", "wactv", "read_t", "writ_t",
        "%r",  "%w", "d/s", "del_t", "zone");
}


type VfsData = Vec<KstatData>;
type ZoneHash = HashMap<i32, KstatData>;

#[derive(Default)]
struct Stats {
    delay_cnt: f64,
    delay_time: f64,
    reads: f64,
    writes: f64,
    nread: f64,
    nwritten: f64,
    rtime: f64,
    wtime: f64,
    rlentime: f64,
    wlentime: f64,
}

static NANOSEC: f64 = 1_000_000_000.0;

// Consume VfsData and return it back as 'instance_id: KstatData'
fn zone_hashmap(data: VfsData) -> ZoneHash {
    data.into_iter().map(|i| (i.instance, i)).collect()
}

// Read a String value from a kstat or panic
fn read_string(data: &KstatNamedData) -> &String {
    match data {
        KstatNamedData::DataString(val) => val,
        _ => panic!("NamedData is not a String"),
    }
}

// Read a u64 value from a kstat or panic
fn read_u64(data: &KstatNamedData) -> u64 {
    match data {
        KstatNamedData::DataUInt64(val) => *val,
        _ => panic!("NamedData is not a u64"),
    }
}

// Get the stats we care about from the KstatData
fn get_stats(data: &HashMap<String, KstatNamedData>) -> Stats {
    let delay_cnt = read_u64(&data["delay_cnt"]) as f64;
    let delay_time = read_u64(&data["delay_time"]) as f64;
    let reads = read_u64(&data["reads"]) as f64;
    let writes = read_u64(&data["writes"]) as f64;
    let nread = read_u64(&data["nread"]) as f64;
    let nwritten = read_u64(&data["nwritten"]) as f64;
    let rtime = read_u64(&data["rtime"]) as f64;
    let wtime = read_u64(&data["wtime"]) as f64;
    let rlentime = read_u64(&data["rlentime"]) as f64;
    let wlentime = read_u64(&data["wlentime"]) as f64;

    Stats {
        delay_cnt,
        delay_time,
        reads,
        writes,
        nread,
        nwritten,
        rtime,
        wtime,
        rlentime,
        wlentime,
    }
}

// Loop over each VfsData and output VFS read/write ops in a meaningful way
fn print_stats(curr: &ZoneHash, old: &Option<ZoneHash>, id: i32, mb: bool, activity: bool,
    all: bool) {
    let mut keys: Vec<_> = curr.keys().collect();
    keys.sort();

    for key in keys {
        let stat = &curr[key];
        let instance = &stat.instance;

        // Only show the current zones info by default
        if !all && id != *instance { continue };

        let zonename = read_string(&stat.data["zonename"]);
        let len = if zonename.len() >= 8 { 8 } else { zonename.len() };
        let zonename = &read_string(&stat.data["zonename"])[0..len];

        let stats = get_stats(&stat.data);
        let old_stats = old.as_ref().map_or(Default::default(), |s| get_stats(&s[instance].data));

        // If a zone appeared during the middle of our run skip it
        if old.is_some() && !old.as_ref().unwrap().contains_key(instance) { continue; };

        let old_snaptime = old.as_ref().map_or(0, |s| s[instance].snaptime);
        let etime = match old_snaptime{
            val if val > 0 => stat.snaptime - old_snaptime,
            _ => stat.snaptime - stat.crtime,
        } as f64 / NANOSEC;

        // TODO: Implement "-I" and set the value here if needed
        let divisor = etime;
        let bytes = if mb { 1024.0 * 1024.0 } else { 1024.0 };

        /*
         * These calculations are transcribed from the perl version of `vfsstat` which was
         * originally written by Brendan Gregg
         */
        let reads = (stats.reads - old_stats.reads) / divisor;
        let writes = (stats.writes - old_stats.writes) / divisor;
        let nread = (stats.nread - old_stats.nread) / divisor / bytes;
        let nwritten = (stats.nwritten - old_stats.nwritten) / divisor / bytes;

        let r_tps = (stats.reads - old_stats.reads) / etime;
        let w_tps = (stats.writes - old_stats.writes) / etime;

        let r_actv = ((stats.rlentime - old_stats.rlentime) / NANOSEC) / etime;
        let w_actv = ((stats.wlentime - old_stats.wlentime) / NANOSEC) / etime;

        let read_t = if r_tps > 0.0 { r_actv * (1000.0 / r_tps) } else { 0.0 } * 1000.0;
        let write_t = if w_tps > 0.0 { w_actv * (1000.0 / w_tps) } else { 0.0 } * 1000.0;

        let delays = stats.delay_cnt - old_stats.delay_cnt;
        let d_tps = delays / etime;
        let del_t = if delays > 0.0 { (stats.delay_time - old_stats.delay_time) / delays } else { 0.0 };

        let r_b_pct = (((stats.rtime - old_stats.rtime) / NANOSEC) / etime) * 100.0;
        let w_b_pct = (((stats.wtime - old_stats.wtime) / NANOSEC) / etime) * 100.0;

        // compare two float values to 1 unit of precision (ulp)
        fn fc(a: f64, b: f64) -> bool {
            a.approx_eq(&b, ::std::f64::EPSILON, 1)
        }

        if !activity || !fc(reads, 0.0) || !fc(writes, 0.0) || !fc(nread, 0.0) ||
            !fc(nwritten, 0.0) {
            write_output!(reads, writes, nread, nwritten, r_actv, w_actv, read_t, write_t, r_b_pct,
                w_b_pct, d_tps, del_t, zonename, instance);
        }
    }
}

fn main() {
    let about = r#"
       The vfsstat utility reports a summary of VFS read and write activity
       per zone.  It first prints all activity since boot, then reports
       activity over a specified interval.

       When run from a non-global zone (NGZ), only activity from that NGZ can
       be observed.  When run from a the global zone (GZ), activity from the
       GZ and all other NGZs can be observed.

       This tool is convenient for examining I/O performance as experienced by
       a particular zone or application.  Other tools which examine solely
       disk I/O do not report reads and writes which may use the filesystem's
       cache.  Since all read and write system calls pass through the VFS
       layer, even those which are satisfied by the filesystem cache, this
       tool is a useful starting point when looking at a potential I/O
       performance problem.  The vfsstat command reports the most accurate
       reading of I/O performance as experienced by an application or zone.

       One additional feature is that ZFS zvol performance is also reported by
       this tool, even though zvol I/O does not go through the VFS layer. This
       is done so that this single tool can be used to monitor I/O performance
       and because its not unreasonable to think of zvols as being included
       along with other ZFS filesystems.

       The calculations and output fields emulate those from iostat(1m) as
       closely as possible.  When only one zone is actively performing disk
       I/O, the results from iostat(1m) in the global zone and vfsstat in the
       local zone should be almost identical.  Note that many VFS read
       operations may be handled by the filesystem cache, so vfsstat and
       iostat(1m) will be similar only when most operations require a disk
       access.

       As with iostat(1m), a result of 100% for VFS read and write utilization
       does not mean that the VFS layer is fully saturated.  Instead, that
       measurement just shows that at least one operation was pending over the
       last interval of time examined.  Since the VFS layer can process more
       than one operation concurrently, this measurement will frequently be
       100% but the VFS layer can still accept additional requests.

       This version is a port of the original vfsstat written by Brendan
       Gregg.

       r/s: reads per second
       w/s: writes per second
       kr/s: kilobytes read per second
       kw/s: kilobytes written per second
       ractv: average number of read operations actively being serviced by the VFS layer
       wactv: average number of write operations actively being serviced by the VFS layer
       read_t: average VFS read latency, in microseconds
       writ_t: average VFS write latency, in microseconds
       %r: percent of time there is a VFS read operation pending
       %w: percent of time there is a VFS write operation pending
       d/s: VFS operations per second delayed by the ZFS I/O throttle
       del_t: average ZFS I/O throttle delay, in microseconds
        "#;
    let matches = App::new("vfsstat")
        .version("0.1.0")
        .author("Mike Zeller <mike@mikezeller.net")
        .about("Report VFS read and write activity")
        .long_about(about)
        .arg(Arg::with_name("H")
            .short("H")
            .help("Don't print the header"))
        .arg(Arg::with_name("z")
            .short("z")
            .help("Hide zones with no VFS activity"))
        .arg(Arg::with_name("Z")
            .short("Z")
            .help("Print results for all zones, not just the current zone"))
        .arg(Arg::with_name("M")
            .short("M")
            .help("Print results in MB/s instead of KB/s"))
        .arg(Arg::with_name("INTERVAL")
            .help("Print results per inverval rather than per second")
            .index(1))
        .arg(Arg::with_name("COUNT")
            .help("Print for n times and exit")
            .required(false)
            .index(2))
        .get_matches();

    let hide_header = matches.is_present("H");
    let use_mb = matches.is_present("M");
    let show_all_zones = matches.is_present("Z");
    let hide_no_activity = matches.is_present("z");

    let interval = match matches.value_of("INTERVAL") {
        None => 1,
        Some(val) => match val.parse::<i32>() {
            Ok(i) => i,
            Err(_) => {
                println!("please provide a valid INTERVAL");
                ::std::process::exit(1);
            }
        }
    };

    let mut count = match matches.value_of("COUNT") {
        None => None,
        Some(val) => match val.parse::<i32>() {
            Ok(i) => Some(i),
            Err(_) => {
                println!("please provide a valid COUNT");
                ::std::process::exit(1);
            }
        }
    };
    if !matches.is_present("INTERVAL") { count = Some(1); };

    let zoneid = zonename::getzoneid().expect("failed to get zoneid");
    let mut header_interval = 0;
    let mut nloops = 0;
    let mut old: Option<ZoneHash> = None;
    let reader =
        KstatReader::new(None, None, None, Some("zone_vfs")).expect("failed to create reader");

    print_header(hide_header);
    loop {
        let stats = reader.read().expect("failed to read kstats");
        let curr = zone_hashmap(stats);

        if header_interval > 20 {
            print_header(hide_header);
            header_interval = 0;
        }
        print_stats(&curr, &old, zoneid, use_mb, hide_no_activity, show_all_zones);
        let _ = ::std::io::stderr().flush();

        // move curr -> old
        old = Some(curr);
        header_interval += 1;

        if count.is_some() {
            nloops += 1;
            if nloops >= *count.as_ref().unwrap() { break; }
        }
        thread::sleep(time::Duration::from_secs(interval as u64));
    }
}
