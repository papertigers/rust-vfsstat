# vfsstat

Report VFS read and write activity

## About

This is a Rust port of the [VFSSTAT(1m)](https://smartos.org/man/1m/vfsstat)
utility that can be found on SmartOS based systems. The original was written
by Brendan Gregg.

The main motivation behind this port is to remove the perl dependency and to
allow lx zones to run the program.

## ToDo
- [ ] Finish implementing the rest of the cli flags


### Usage

```
vfsstat 0.1.0
Mike Zeller <mike@mikezeller.net

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


USAGE:
    vfsstat [FLAGS] [ARGS]

FLAGS:
    -H               Don't print the header
    -Z               Print results for all zones, not just the current zone
    -h, --help       Prints help information
    -V, --version    Prints version information
    -z               Hide zones with no VFS activity

ARGS:
    <INTERVAL>    Print results per inverval rather than per second
    <COUNT>       Print for n times and exit
```

```
root@plex:~# uname -a
Linux plex 4.3.0 BrandZ virtual linux x86_64 x86_64 x86_64 GNU/Linux
root@plex:~# ./vfsstat 1
  r/s   w/s  kr/s  kw/s ractv wactv read_t writ_t     %r     %w    d/s  del_t zone
6955.8  93.8 73657.2 144.1   0.0   0.0    5.6   13.7    2.5    0.1    8.5   45.7 67306221 (14)
8496.6   0.0 271891.7   0.0   0.1   0.0    6.7    0.0    2.9    0.0    0.0    0.0 67306221 (14)
8480.0   0.0 271361.1   0.0   0.1   0.0    6.8    0.0    2.9    0.0    0.0    0.0 67306221 (14)
8495.9   0.0 271870.2   0.0   0.1   0.0    6.7    0.0    2.9    0.0    0.0    0.0 67306221 (14)
8485.4   2.0 271532.1   0.3   0.1   0.0    6.8   44.5    2.9    0.0    0.0    0.0 67306221 (14)
8493.6   0.0 271795.4   0.0   0.1   0.0    6.8    0.0    2.9    0.0    0.0    0.0 67306221 (14)
8485.9   0.0 271547.2   0.0   0.1   0.0    6.9    0.0    3.0    0.0    0.0    0.0 67306221 (14)
8467.5   4.0 270961.1   0.5   0.1   0.0    7.0   22.0    3.0    0.0    0.0    0.0 67306221 (14)
```
