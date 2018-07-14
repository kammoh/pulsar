

macro_rules! mycat {
    ($($line: expr)+,)  =>   (           
            concat!(
                $(
                    concat!("\n\t", $line), 
                )+
            )
    )
}

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
                    "\n\tclflush  ($0)"
                )
                :
                :  "r" ($addr)
                : "volatile" 
            )
        }
    )
}

#[inline]
fn rdtscp() -> u64 {
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
fn maccess(p: &u64) {
    unsafe {
        asm!(
            "movq (%0), %%rax\n"
            :
            : "r" (p)
            : "%rax", "%rbx", "%rcx", "%rdx"
            : "volatile"
        )
    }
}

#[inline]
fn flush(p: &u64) {
    clflush!(p);
}



macro_rules! reload_flush {
    ($addr: expr, $time_stamp: expr, $delta: expr)    =>   (
        unsafe {
            asm!(
                concat!(
                    "\n\tmfence",
                    "\n\tlfence",
                    "\n\trdtscp",
                    "\n\tshl $$32, %rdx",
                    "\n\tor  %rdx, %rax",
                    "\n\tmov %rax, %rsi",
                    "\n\tmovq (%rbx), %rax",
                    "\n\trdtscp",
                    "\n\tshl $$32, %rdx",
                    "\n\tor  %rdx, %rax",
                    "\n\tmov %rax, %rdx",
                    "\n\tsub %rsi, %rax",
                    "\n\tclflush  0(%rbx)"
                )
                : "={rax}" (*$delta), "={rdx}"(*$time_stamp)   /* Outputs $1 = time_stamp */
                : "{rbx}" ($addr)           /* Inputs: $2 = addr */
                : "rsi", "rcx"      /* Clobbers */
                : "volatile"           /* Options  */
            );
        }
    )
}

use std::ptr::NonNull;

pub struct Monitor {
    pub addr: *const u8,
    pub hit_ts: Vec<u64>,
}

unsafe impl std::marker::Send for Monitor {}

use rand::prelude::*;
use std::ptr;

fn run_thread(fire_tsc: u64, mut mon: Monitor, threshold: u64, timeout: u64) -> Monitor {
    let mut ts: u64 = 0;
    let mut delta: u64 = 0;
    let max_samples = mon.hit_ts.capacity() - 1;
    let addr = mon.addr;
    let end_tsc = fire_tsc + timeout;

 
    // let p = v.as_mut_ptr();

    clflush!(addr);

    while rdtscp() < fire_tsc {
        unsafe {
            asm!("nop":::: "volatile");
        }
    }

    eprintln!("in run_thread end_tsc= {}", fire_tsc);

    loop {
        loop {
            reload_flush!(addr, &mut ts, &mut delta);
            if delta < threshold {
                break;
            }
            if ts > end_tsc {
                 eprintln!("[timeout] end of run_thread {}", mon.hit_ts.len());

                return mon;
            }
        }
        // unsafe {
        //     ptr::write(p.offset(i), ts);
        // }
        mon.hit_ts.push(ts);
    }

    eprintln!("end of run_thread {}", mon.hit_ts.len());

    mon
}

pub fn fr(monitors:&mut  Vec<Monitor>, threshold: u64, timeout: u64) {


    // let mut rng = thread_rng();

    // let sleep_timespec = libc::timespec {
    //     tv_sec: (useconds / 1000000),
    //     tv_nsec: (useconds % 1000000) * 1000,
    // };

    for m in monitors.iter() {
        clflush!(m.addr);
    }

    use std::thread;

    let allowed_core_ids = core_affinity::get_core_ids().unwrap();

    let mut handles = Vec::new();

    let now = rdtscp();

    let fire_time = now + 200000; // TODO 
    
    for (idx, mon) in monitors.drain(..).enumerate() {
        let id = allowed_core_ids[idx % allowed_core_ids.len()];

        let handle = thread::spawn(move || {
            core_affinity::set_for_current(id);
            run_thread(fire_time, mon, threshold, timeout)
        });
        handles.push(handle);
    }

    for handle in handles.into_iter() {
        monitors.push(handle.join().unwrap());
    }
}