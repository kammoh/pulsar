use rand::prelude::*;
use std::thread;
use super::flush_reload::*;
use super::attack::*;
use thread_priority::*;
use std::sync::{Arc, Barrier};
use std::cmp;


pub fn histogram(attack: Attack, do_print: bool) -> u64 {

    let len: usize = 8*1024;
    let mut rng = thread_rng();

    
    let access_array: Vec<u64> = (0..len).map(|_| {rng.gen_range(0, len as u64)}).collect();
    
    let hit_hi: usize = 16; // len; // (len/2 - 16)
    let indirect_index: Vec<u64> = (0..len).map(|_| { (rng.gen_range(0, hit_hi/16) * 16 + rng.gen_range(0,2) * (128) ) as u64}).collect();


    let hist_mx = 600;
    let mut miss_hist = vec![0;hist_mx];
    let mut hit_hist = vec![0;hist_mx];
    let mut delta: u64 = 0;
    let mut tsc: u64 = 0;
    let barrier = Arc::new(Barrier::new(2));
    let barrier_cloned = barrier.clone();

    let cores = core_affinity::get_core_ids().unwrap();

    let l = hist_mx - 1;

    {
        core_affinity::set_for_current(cores[0]);
    }

    let p = MyBox(&access_array[0] as *const u64);

    for i in 0..len {
        flush(&access_array[i]);
        flush(&indirect_index[i]);
    }


    let end_tsc = rdtscp() + 20000 * (2300 * 1000)/*milli*/;

    let t_handle = thread::spawn( move || {

        core_affinity::set_for_current(cores[1]);

        
        let q = p.0;

        let mut i = 0;

        while i < hit_hi /*(len/2) - 16*/ {
            mem_access(unsafe {&*(q.offset(i as isize))});
            i+=8;
        }

        i = 0;
        while i < hit_hi /*(len/2) - 16*/ {
            mem_access(unsafe {&*(q.offset(i as isize))});
            i+=2;
        }

        let mut rng = thread_rng();
        
        barrier_cloned.wait();

        while rdtscp() < end_tsc {
            
            // let i = rng.gen_range(0, hit_hi/8) * 8;
            // // while i < hit_hi /*(len/2) - 16*/ {
            //     mem_access(unsafe {&*(q.offset(i as isize))});
            // //     i+=8;
            // // }

            for i in 0..(hit_hi/8) {
                mem_access(unsafe {&*(q.offset( (i*8) as isize))});
            }

        }
    });
    
    

    let mut hit_sum = 0;
    let mut miss_sum = 0;

    set_thread_priority(
        thread_native_id(),
        ThreadPriority::Max,
        ThreadSchedulePolicy::Normal (NormalThreadSchedulePolicy::Normal)
    ).unwrap();


    barrier.wait();
    tsc_wait(2300*100);

    let mut bad_l1_hits = 0;


    while tsc < end_tsc {
        let idx1 = ( (tsc + 113 ) as usize) % len; //rng.gen_range(0, len);

        let addr = &access_array[indirect_index[idx1] as usize] as *const u64 as *const u8;
            
        match attack {
            Attack::FlushReload => {
                reload_flush(addr, &mut tsc, &mut delta)
            }
            Attack::FlushFlush => {
                time_clflush(addr, &mut tsc, &mut delta);
            }
        }
        
        
        // if idx1 < (len/2 - 16) as usize {
        //     hit_hist[cmp::min(l, delta as usize)]+=1;
        //     hit_sum+=delta;
        // } else if idx1 > (len/2 +16) as usize {
        //     miss_hist[cmp::min(l , delta as usize)]+=1;
        //     miss_sum+=delta;
        // }

        if indirect_index[idx1] < hit_hi as u64 {
            hit_hist[cmp::min(l, delta as usize)]+=1;
            hit_sum+=delta;
        } else { // should be miss
            if delta < 100 {
                bad_l1_hits += 1;
                // eprintln!("delta={} idx={} indirect_index[idx1]={}", delta, idx1, indirect_index[idx1]);
            }
            miss_hist[cmp::min(l , delta as usize)]+=1;
            miss_sum+=delta;
        }

        // thread::sleep(std::time::Duration::from_micros(100));  
        
    }

    t_handle.join().unwrap();

    let total_hits = hit_hist.iter().sum::<u64>();
    let total_misses = miss_hist.iter().sum::<u64>();

    let u_hit = hit_sum/total_hits;
    let u_miss = miss_sum/total_misses;
    let u_delta = match attack {
         Attack::FlushFlush => {u_hit - u_miss}
         _ => {u_miss - u_hit}
    };


    let mut min_array = Vec::new();

    match attack {
         Attack::FlushFlush => {min_array.push(total_misses);}
         _ => {min_array.push(total_hits);}
    };

    

    eprintln!("u_hit={} u_miss={} delta={} bad_l1_hits={}", u_hit, u_miss, u_delta, bad_l1_hits);
    for i in 0..hist_mx {
        
        let last = min_array.last().unwrap().clone();
        let n = match attack {
            Attack::FlushFlush => {last + hit_hist[i] - miss_hist[i]}
            _ => {last + miss_hist[i] - hit_hist[i]}
        };

        min_array.push(n);
        if do_print {
            println!("{}: {} {}", i, (100. * hit_hist[i] as f64) / (total_hits as f64) , (100. * miss_hist[i] as f64) / (total_misses as f64) );
        }
    }

    let (bads, optimal_threshold) = min_array.iter().enumerate().map(|(x, y)| (y, x)).min().unwrap();

    eprintln!("optimal_threshold={} error={:.3}%", optimal_threshold,  (100. * *bads as f64)/((total_misses + total_hits) as f64) );


    // println!("hit_flush:{} miss_flush{}", (hit_sum as f64) / (n as f64), (miss_sum as f64) / (n as f64) );
    optimal_threshold as u64
}

pub fn histogram_monitor(monitors:&mut  Vec<Monitor>, attack: Attack, threshold: u64, do_print: bool) -> u64 {

    
    let len: usize = 8*1024;
    let mut rng = thread_rng();
    
    let indirect_index: Vec<usize> = (0..len).map(|_| { (rng.gen_range(0, monitors.len()) ) as usize}).collect();


    let hist_mx = 600;
    let mut hit_hist = vec![vec![0;hist_mx]; monitors.len()];
    let mut delta: u64 = 0;
    let mut tsc: u64 = 0;

    let cores = core_affinity::get_core_ids().unwrap();

    let l = hist_mx - 1;

    {
        core_affinity::set_for_current(cores[0]);
    }


    let end_tsc = rdtscp() + 4000 * (2300 * 1000)/*milli*/;


    let mut hit_sum = 0;
    let mut miss_sum = 0;

    set_thread_priority(
        thread_native_id(),
        ThreadPriority::Max,
        ThreadSchedulePolicy::Normal (NormalThreadSchedulePolicy::Normal)
    ).unwrap();


    tsc_wait(2300*100);

    let mut bad_l1_hits = 0;


    while tsc < end_tsc {
        let idx1 = ( (tsc) as usize) % len; //rng.gen_range(0, len);

        let mon = &mut monitors[indirect_index[idx1] as usize];

        let addr = mon.addr;
            
        // match attack {
        //     Attack::FlushReload(_) => {
        //         reload_flush(addr, &mut tsc, &mut delta)
        //     }
        //     Attack::FlushFlush(_) => {
                time_clflush(addr, &mut tsc, &mut delta);
        //     }
        // }
        
        
        // if idx1 < (len/2 - 16) as usize {
        //     hit_hist[cmp::min(l, delta as usize)]+=1;
        //     hit_sum+=delta;
        // } else if idx1 > (len/2 +16) as usize {
        //     miss_hist[cmp::min(l , delta as usize)]+=1;
        //     miss_sum+=delta;
        // }

            if delta > threshold && delta < 2*threshold {
                mon.hit_ts.push(tsc);
                hit_hist[indirect_index[idx1]] [cmp::min(l, delta as usize)]+=1;
            }
        // }

        // thread::sleep(std::time::Duration::from_micros(100));  
        
    }


    // let total_hits: Vec<u64> = hit_hist.iter().map(|v| v.iter().sum()).collect();
    // let total_misses = miss_hist.iter().sum::<u64>();

    // let u_hit = hit_sum/total_hits;
    // let u_miss = miss_sum/total_misses;
    // let u_delta = match attack {
    //      Attack::FlushFlush(_) => {u_hit - u_miss}
    //      _ => {u_miss - u_hit}
    // };


    // let mut min_array = Vec::new();

    // match attack {
    //      Attack::FlushFlush(_) => {min_array.push(total_misses);}
    //      _ => {min_array.push(total_hits);}
    // };

    

    // eprintln!("u_hit={} u_miss={} delta={} bad_l1_hits={}", u_hit, u_miss, u_delta, bad_l1_hits);
    // for i in 0..hist_mx {
    //     // let n = min_array.last().unwrap() + match attack {
    //     //     Attack::FlushFlush(_) => {hit_hist[i] - miss_hist[i]}
    //     //     _ => {miss_hist[i] - hit_hist[i]}
    //     // };

    //     // min_array.push(n);
    //     // if  do_print {
    //         print!("{}: ", i );
    //         for (idx,h) in hit_hist.iter().enumerate() {
    //             print!("{} ", (100. * h[i] as f64) / (total_hits[idx] as f64)  );
    //         }
    //         println!("")
    //     // }
    // }

    0

    // let (bads, optimal_threshold) = min_array.iter().enumerate().map(|(x, y)| (y, x)).min().unwrap();

    // eprintln!("optimal_threshold={} error={:.3}%", optimal_threshold,  (100. * *bads as f64)/((total_misses + total_hits) as f64) );


    // // println!("hit_flush:{} miss_flush{}", (hit_sum as f64) / (n as f64), (miss_sum as f64) / (n as f64) );
    // optimal_threshold as u64
}