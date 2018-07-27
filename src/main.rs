#![allow(dead_code)]
// Opt in to unstable features expected for Rust 2018
#![feature(rust_2018_preview)]
// Opt in to warnings about new 2018 idioms
// #![warn(rust_2018_idioms)]
#![feature(asm)]

// extern crate raw_cpuid;

mod symbol;
mod cbindings;
mod attack;
mod flush_reload;
mod histogram;

use raw_cpuid::CpuId;
use std::error::Error;

use self::symbol::*;
use self::flush_reload::*;
use self::attack::*;
use self::histogram::*;

use std::collections::BTreeMap;
use std::{thread, time};
use clap::*;
use std::result::Result;

fn get_cache_info() -> Result<(), Box<dyn Error>> {
    let cpuid = CpuId::new();
    {
        let vf = cpuid.get_feature_info().unwrap();
        eprintln!(
            "{} {} cache-line size= {} bytes ",
            vf.has_clflush(),
            cpuid.get_extended_feature_info().unwrap().has_clflushopt(),
            vf.cflush_cache_line_size() * 8
        );

        match cpuid.get_cache_parameters() {
            Some(cparams) => {
                for cache in cparams {
                    let size = cache.associativity()
                        * cache.physical_line_partitions()
                        * cache.coherency_line_size()
                        * cache.sets();
                    eprintln!(
                        "L{}\n\tsize:{}\n\t assoc:{}\n\t inclusive:{}",
                        cache.level(),
                        size,
                        cache.associativity(),
                        cache.is_inclusive()
                    );
                }
            }
            None => eprintln!("No cache parameter information available"),
        }

        Ok(())
    }
}


fn main() -> Result<(), i32> {

    let attack_arg = Arg::with_name("attack")
                        .help("Type of attack")
                        .possible_values(&["ff", "fr"])
                        .required(true);

    let claps = App::new("Pulsar")
                        // .author("Kamyar Moh <kammoh@gmail.com>")
                        .about("A Cache Side Channel Attack Framework")
                        .subcommand(SubCommand::with_name("attack")
                                                .about("Run attack")
                                                .arg(attack_arg.clone())
                                                .args_from_usage("
                                                        <binary>
                                                        <threshold>
                                                        <timeout>
                                                        [delay] 'wait (secs) before start'")
                                                .arg(Arg::with_name("monitors")
                                                    .multiple(true)
                                                    .last(true)))
                        .subcommand(SubCommand::with_name("hist")
                                                .about("Run histogram")
                                                .arg(attack_arg.clone()))
                        .get_matches();




    match claps.subcommand() {
        ("cache-info", Some(claps)) => {
            get_cache_info().or(Err(-1))
        }
        ("attack", Some(claps)) => {

            let attack = match claps.value_of("attack") {
                Some("ff") => {Attack::FlushFlush}
                Some("fr") => {Attack::FlushReload}
                _ => {panic!("unknown attack")}
            };

            let file_name = value_t_or_exit!(claps, "binary", String);
            let threshold = value_t_or_exit!(claps, "threshold", u64);
            let timeout = value_t_or_exit!(claps, "timeout", u64);
            let delayed = value_t!(claps, "delay", u64).unwrap_or(0);
            let monitor_names = values_t_or_exit!(claps, "monitors", String);
            
            let mut monitors = Vec::new();

            for mn in monitor_names {
                let r = get_symbol_offset(&file_name, &mn).unwrap();

                let addr = map_offset(&file_name, r.clone());

                eprintln!("{} {:X} {:?}", mn, r, addr);
                monitors.push(Monitor{addr, hit_ts: Vec::with_capacity(2048)});
            }

            for i in 0..delayed {
                eprintln!("{}...", delayed - i);
                thread::sleep(time::Duration::from_secs(1));
            }

            histogram_monitor(&mut monitors, attack, threshold, true);

            // run_attack(&mut monitors, attack, threshold, timeout);

            let mut map = BTreeMap::new();

            for (idx, m) in monitors.iter().enumerate() {
                eprintln!("monitor {} samples: {}", idx, m.hit_ts.len());
                for hit_ts in m.hit_ts.iter() {
                    map.insert(hit_ts, idx);
                }
            }

            if map.len() > 1 {
                let (init_ts, _) = map.iter().nth(0).unwrap();

                for (ts, mon) in map.iter() {
                    println!("{}, {}", *ts - *init_ts, mon);
                }
            }
            Ok(())
        }
        ("hist", Some(claps)) => {

            let attack = match claps.value_of("attack") {
                Some("ff") => {Attack::FlushFlush}
                Some("fr") => {Attack::FlushReload}
                _ => {panic!("x unknown attack")}
            };

            histogram(attack, true);
            Ok(())
        }
        _ => {Err(-1)}
    }

}
