#![allow(dead_code)]
// Opt in to unstable features expected for Rust 2018
#![feature(rust_2018_preview)]
// Opt in to warnings about new 2018 idioms
// #![warn(rust_2018_idioms)]
#![feature(asm)]

// extern crate raw_cpuid;

mod symbol;
mod cbindings;
mod flush_reload;

use raw_cpuid::CpuId;
use std::error::Error;

use self::symbol::*;
use self::flush_reload::*;
use std::collections::BTreeMap;
use std::ptr::NonNull;

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
    let args: Vec<String> = ::std::env::args().collect();

    if args.len() < 5 {
        eprintln!("Usage: pulsar <binary> <threshold> <timeout> monitors");
        return Err(-1);
    }

    get_cache_info().expect("get cache failed");

    let file_name = &args[1];
    let threshold = args[2].parse::<u64>().unwrap();
    let timeout = args[3].parse::<u64>().unwrap();
    
    let monitor_names = &args[4..]; // vec!["mpih-mul.c:90", "mpihelp_divrem", "mpih-mul.c:270"];
    
    let mut monitors = Vec::new();



    for mn in monitor_names {
        let r = get_symbol_offset(file_name, mn).unwrap();

        let addr = map_offset(file_name, r.clone());

        eprintln!("{} {:X} {:?}", mn, r, addr);
        monitors.push(Monitor{addr, hit_ts: Vec::with_capacity(2048)});
    }

    
    

    fr(&mut monitors, threshold, timeout);

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
