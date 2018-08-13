use rand::prelude::*;
use std::thread;
use super::flush_reload::*;
use super::attack::*;
use thread_priority::*;
use std::sync::{Arc, Barrier};
use std::cmp;

struct Timestamp {
    pub tsc: u64,
    pub value: i64
}


pub fn histogram(attack: Attack, do_print: bool) -> u64 {

    let len: usize = 1*1024/8;
    let mut rng = thread_rng();

    let access_array: Vec<u64> = (0..len).map(|_| {rng.gen_range(0, len as u64)}).collect();



    let barrier = Arc::new(Barrier::new(2));
    let barrier_cloned = barrier.clone();

    let cores = core_affinity::get_core_ids().unwrap();

    {
        core_affinity::set_for_current(cores[0]);
    }

    let p = MyBox(&access_array[0] as *const u64);



    let end_tsc = rdtscp() + 4 * 1000 * (2300 * 1000)/*milli*/;

    let t_handle = thread::spawn( move || {
        core_affinity::set_for_current(cores[2]);

        let mut rng = thread_rng();
        // let mut x = 0;

        let mut time_stamps = Vec::with_capacity(20000);



        let q = p.0;

        let indir: Vec<isize> = (0..64).map(|_|rng.gen_range(0,3)).collect();

        barrier_cloned.wait();

        let mut tsc = 0;

        while tsc < end_tsc {
            let i = (tsc as usize) % indir.len(); 
            tsc = rdtscp();
            for _ in 0..10000 {
                mem_access(unsafe {&*(q.offset( indir[i] * (len/3) as isize))});

                // x += unsafe {*(q.offset(  indir[i] * (len/3) as isize))};
            }
            time_stamps.push(Timestamp {tsc, value: indir[i] as i64 });
        }

        // eprintln!("x={} ",x);

        time_stamps
    });



    let mut hit_sum = 0;
    let mut miss_sum = 0;

    set_thread_priority(
        thread_native_id(),
        ThreadPriority::Max,
        ThreadSchedulePolicy::Normal (NormalThreadSchedulePolicy::Normal)
    ).unwrap();



    let mut bad_l1_hits = 0;

    let mut dummy_vec = Vec::new();

    for _ in 0..64{
        dummy_vec.push(rng.gen_range(0,256) as u8);
    }


    barrier.wait();
    tsc_wait(2300*100);

    flush(unsafe {&*(&dummy_vec[0] as *const u8 as *const u64)});


    let mut observed_times = Vec::new();

    let mut tsc = 0;
    let mut delta = 0;


    let indir: Vec<usize> = (0..64).map(|_|rng.gen_range(0,3)).collect();

    while tsc < end_tsc {

        let i = (tsc as usize + 17)  % indir.len();
        let addr = &access_array[indir[ i ] * (len/3)] as *const u64 as *const u8;

        match attack {
            Attack::FlushReload => {
                reload_flush(addr, &mut tsc, &mut delta)
            }
            Attack::FlushFlush => {
                time_clflushx(addr, (& addr) as *const *const u8 as *const u8, &mut tsc, &mut delta);
            }
        }
        observed_times.push(Timestamp { tsc, value: delta })

    }

    let time_stamps = t_handle.join().unwrap();

    let hist_mx = 600;
    let mut miss_hist = vec![vec![0;hist_mx];3];
    let mut hit_hist = vec![vec![0;hist_mx];3];

//    let total_hits = hit_hist.iter().sum::<u64>();
//    let total_misses = miss_hist.iter().sum::<u64>();
//
//    let u_hit = hit_sum/total_hits;
//    let u_miss = miss_sum/total_misses;
//    let u_delta = match attack {
//        //  Attack::FlushFlush => {u_hit - u_miss}
//         _ => 1//{u_miss - u_hit}
//    };


//    let mut min_array = Vec::new();

//    match attack {
//         Attack::FlushFlush => {min_array.push(total_misses);}
//         _ => {min_array.push(total_hits);}
//    };



//    eprintln!("u_hit={} u_miss={} delta={} bad_l1_hits={}", u_hit, u_miss, u_delta, bad_l1_hits);

    let mut time_stamps_idx = 0;

    for ob in observed_times.iter() {
        while time_stamps_idx < time_stamps.len() && time_stamps[time_stamps_idx].tsc <= ob.tsc {
            time_stamps_idx += 1;
        }

        let ground_truth = time_stamps[time_stamps_idx - 1].value as usize;

        let hist_idx = cmp::max(0, cmp::min(ob.value.abs() as usize, hist_mx - 1));

        for i in 0..3 {
            if i == ground_truth {
                hit_hist[i][hist_idx] += 1;
            } else {
                miss_hist[i][hist_idx] += 1;
            }
        }

    }


    for val in 0..hist_mx {

//        let last = min_array.last().unwrap().clone();
//        let n = match attack {
//            Attack::FlushFlush => {last + hit_hist[val] - miss_hist[val]}
//            _ => {last + miss_hist[val] - hit_hist[val]}
//        };
//
//        min_array.push(n);
        print!("{}: ", val);
        for i in 0..3 {
            print!("{} {} ", hit_hist[i][val], miss_hist[i][val]);
        }
        print!("\n");
    }

//    let (bads, optimal_threshold) = min_array.iter().enumerate().map(|(x, y)| (y, x)).min().unwrap();
//
//    eprintln!("optimal_threshold={} error={:.3}%", optimal_threshold,  (100. * *bads as f64)/((total_misses + total_hits) as f64) );
//

    // println!("hit_flush:{} miss_flush{}", (hit_sum as f64) / (n as f64), (miss_sum as f64) / (n as f64) );
//    optimal_threshold as u64
    0
}


pub fn histogram_monitor(monitors: &mut Vec<Monitor>, attack: Attack, threshold: u64, do_print: bool) -> u64 {


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

        let mut miss = 0;

        for _ in 0..10 {
            if delta < threshold {
                miss += 1;
            }
            tsc_wait(1000);
        }
            mon.hit_ts.push(tsc);
            hit_hist[indirect_index[idx1]] [cmp::min(l, delta as usize)]+=1;
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