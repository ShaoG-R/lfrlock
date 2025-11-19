use criterion::{criterion_group, criterion_main, Criterion};
use lfrlock::LfrLock;
use arc_swap::ArcSwap;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
struct Data(Vec<u32>);

// 1. Pure Read Performance
fn read_only_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_only_single_thread");
    
    let lock = LfrLock::new(Data(vec![0; 10]));
    group.bench_function("LfrLock", |b| {
        b.iter(|| {
            let local_epoch = lock.register();
            let guard = local_epoch.pin();
            let _ = lock.read(&guard).0[0];
        })
    });

    let lock = ArcSwap::from_pointee(Data(vec![0; 10]));
    group.bench_function("ArcSwap", |b| {
        b.iter(|| {
            let _ = lock.load().0[0];
        })
    });

    let lock = Mutex::new(Data(vec![0; 10]));
    group.bench_function("Mutex", |b| {
        b.iter(|| {
            let _ = lock.lock().unwrap().0[0];
        })
    });
    
    group.finish();
}

// 2. Few writes, many reads (Read Heavy Concurrent)
// This simulates the traditional advantage of arc-swap (and RCU-like structures)
fn read_heavy_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_heavy_concurrent");
    // Increase sample size/time if needed, but for concurrent tests keep it reasonable
    group.sample_size(10);
    
    let num_readers = 4;
    let ops_per_thread = 10_000; 
    // 1 writer doing 1/10th of operations implies very read heavy

    // LfrLock
    group.bench_function("LfrLock", |b| {
        b.iter(|| {
            let lock = LfrLock::new(Data(vec![0; 10]));
            let mut handles = vec![];
            
            // Readers
            for _ in 0..num_readers {
                let lock = lock.clone();
                handles.push(thread::spawn(move || {
                    let local_epoch = lock.register();
                    for _ in 0..ops_per_thread {
                        let guard = local_epoch.pin();
                        let _ = lock.read(&guard).0[0];
                    }
                }));
            }
            
            // 1 Writer
            let lock_w = lock.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..(ops_per_thread/10) { 
                     lock_w.update(Data(vec![0; 10]));
                }
            }));

            for h in handles { h.join().unwrap(); }
        })
    });

    // ArcSwap
    group.bench_function("ArcSwap", |b| {
        b.iter(|| {
            let lock = Arc::new(ArcSwap::from_pointee(Data(vec![0; 10])));
            let mut handles = vec![];

            // Readers
            for _ in 0..num_readers {
                let lock = lock.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        let _ = lock.load().0[0];
                    }
                }));
            }
            
            // 1 Writer
            let lock_w = lock.clone();
            handles.push(thread::spawn(move || {
                 for _ in 0..(ops_per_thread/10) {
                     lock_w.store(Arc::new(Data(vec![0; 10])));
                 }
            }));

            for h in handles { h.join().unwrap(); }
        })
    });

    // Mutex
    group.bench_function("Mutex", |b| {
        b.iter(|| {
            let lock = Arc::new(Mutex::new(Data(vec![0; 10])));
            let mut handles = vec![];

            // Readers
            for _ in 0..num_readers {
                let lock = lock.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        let _ = lock.lock().unwrap().0[0];
                    }
                }));
            }
            
            // 1 Writer
            let lock_w = lock.clone();
            handles.push(thread::spawn(move || {
                 for _ in 0..(ops_per_thread/10) {
                     *lock_w.lock().unwrap() = Data(vec![0; 10]);
                 }
            }));

            for h in handles { h.join().unwrap(); }
        })
    });

    group.finish();
}

// 3. Write Heavy Concurrent (Multi-write Multi-read)
fn write_heavy_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_heavy_concurrent");
    group.sample_size(10);
    
    let num_pairs = 4; // 4 readers, 4 writers
    let ops_per_thread = 1_000; // Reduce ops to keep benchmark duration sane

    // LfrLock
    group.bench_function("LfrLock", |b| {
        b.iter(|| {
            let lock = LfrLock::new(Data(vec![0; 10]));
            let mut handles = vec![];
            
            for _ in 0..num_pairs {
                // Reader
                let lock_r = lock.clone();
                handles.push(thread::spawn(move || {
                    let local_epoch = lock_r.register();
                    for _ in 0..ops_per_thread {
                        let guard = local_epoch.pin();
                        let _ = lock_r.read(&guard).0[0];
                    }
                }));
                
                // Writer
                let lock_w = lock.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        lock_w.update(Data(vec![0; 10]));
                    }
                }));
            }
            for h in handles { h.join().unwrap(); }
        })
    });

    // ArcSwap
    group.bench_function("ArcSwap", |b| {
         b.iter(|| {
            let lock = Arc::new(ArcSwap::from_pointee(Data(vec![0; 10])));
            let mut handles = vec![];
            
            for _ in 0..num_pairs {
                // Reader
                let lock_r = lock.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        let _ = lock_r.load().0[0];
                    }
                }));
                
                // Writer
                let lock_w = lock.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        lock_w.store(Arc::new(Data(vec![0; 10])));
                    }
                }));
            }
            for h in handles { h.join().unwrap(); }
        })
    });

    // Mutex
    group.bench_function("Mutex", |b| {
        b.iter(|| {
            let lock = Arc::new(Mutex::new(Data(vec![0; 10])));
            let mut handles = vec![];
            
            for _ in 0..num_pairs {
                // Reader
                let lock_r = lock.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        let _ = lock_r.lock().unwrap().0[0];
                    }
                }));
                
                // Writer
                let lock_w = lock.clone();
                handles.push(thread::spawn(move || {
                    for _ in 0..ops_per_thread {
                        *lock_w.lock().unwrap() = Data(vec![0; 10]);
                    }
                }));
            }
            for h in handles { h.join().unwrap(); }
        })
    });

    group.finish();
}

criterion_group!(benches, read_only_single_thread, read_heavy_concurrent, write_heavy_concurrent);
criterion_main!(benches);
