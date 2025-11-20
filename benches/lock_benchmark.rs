use arc_swap::ArcSwap;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use lfrlock::LfrLock;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
struct Data(Vec<u32>);

// 1. Pure Read Performance
fn read_only_single_thread(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_only_single_thread");

    let lock = LfrLock::new(Data(vec![0; 10]));
    // LfrLock
    group.bench_function("LfrLock", |b| {
        b.iter(|| {
            let _ = lock.read().0[0];
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

// 2. Read Heavy Concurrent with varying ratios
// Ratios: 1/1000, 1/100, 1/10 (Writes / Reads)
fn read_heavy_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("read_heavy_concurrent");
    // Increase sample size/time if needed, but for concurrent tests keep it reasonable
    group.sample_size(10);

    let num_readers = 4;
    let ops_per_thread = 10_000;

    for write_ratio in [1000, 100, 10].iter() {
        let ratio_str = format!("1:{}", write_ratio);
        let num_writes = ops_per_thread / write_ratio;

        // LfrLock
        group.bench_with_input(
            BenchmarkId::new("LfrLock", &ratio_str),
            &num_writes,
            |b, &num_writes| {
                b.iter(|| {
                    let lock = LfrLock::new(Data(vec![0; 10]));
                    let mut handles = vec![];

                    // Readers
                    for _ in 0..num_readers {
                        let lock = lock.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                let _ = lock.read().0[0];
                            }
                        }));
                    }

                    // 1 Writer
                    let lock_w = lock.clone();
                    handles.push(thread::spawn(move || {
                        for _ in 0..num_writes {
                            lock_w.update(Data(vec![0; 10]));
                        }
                    }));

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );

        // ArcSwap
        group.bench_with_input(
            BenchmarkId::new("ArcSwap", &ratio_str),
            &num_writes,
            |b, &num_writes| {
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
                        for _ in 0..num_writes {
                            lock_w.store(Arc::new(Data(vec![0; 10])));
                        }
                    }));

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );

        // Mutex
        group.bench_with_input(
            BenchmarkId::new("Mutex", &ratio_str),
            &num_writes,
            |b, &num_writes| {
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
                        for _ in 0..num_writes {
                            *lock_w.lock().unwrap() = Data(vec![0; 10]);
                        }
                    }));

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

// 3. Write Heavy Concurrent (Multi-write Multi-read)
// Ratios (Readers:Writers): 16:4, 8:4, 4:4
fn write_heavy_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("write_heavy_concurrent");
    group.sample_size(10);

    let num_writers = 4;
    let ops_per_thread = 1_000; // Reduce ops to keep benchmark duration sane

    for num_readers in [16, 8, 4].iter() {
        let config_str = format!("{}R:{}W", num_readers, num_writers);

        // LfrLock
        group.bench_with_input(
            BenchmarkId::new("LfrLock", &config_str),
            num_readers,
            |b, &num_readers| {
                b.iter(|| {
                    let lock = LfrLock::new(Data(vec![0; 10]));
                    let mut handles = vec![];

                    // Readers
                    for _ in 0..num_readers {
                        let lock_r = lock.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                let _ = lock_r.read().0[0];
                            }
                        }));
                    }

                    // Writers
                    for _ in 0..num_writers {
                        let lock_w = lock.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                lock_w.update(Data(vec![0; 10]));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );

        // ArcSwap
        group.bench_with_input(
            BenchmarkId::new("ArcSwap", &config_str),
            num_readers,
            |b, &num_readers| {
                b.iter(|| {
                    let lock = Arc::new(ArcSwap::from_pointee(Data(vec![0; 10])));
                    let mut handles = vec![];

                    // Readers
                    for _ in 0..num_readers {
                        let lock_r = lock.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                let _ = lock_r.load().0[0];
                            }
                        }));
                    }

                    // Writers
                    for _ in 0..num_writers {
                        let lock_w = lock.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                lock_w.store(Arc::new(Data(vec![0; 10])));
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );

        // Mutex
        group.bench_with_input(
            BenchmarkId::new("Mutex", &config_str),
            num_readers,
            |b, &num_readers| {
                b.iter(|| {
                    let lock = Arc::new(Mutex::new(Data(vec![0; 10])));
                    let mut handles = vec![];

                    // Readers
                    for _ in 0..num_readers {
                        let lock_r = lock.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                let _ = lock_r.lock().unwrap().0[0];
                            }
                        }));
                    }

                    // Writers
                    for _ in 0..num_writers {
                        let lock_w = lock.clone();
                        handles.push(thread::spawn(move || {
                            for _ in 0..ops_per_thread {
                                *lock_w.lock().unwrap() = Data(vec![0; 10]);
                            }
                        }));
                    }

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

// 4. Creation and Cloning Performance
fn bench_creation_and_cloning(c: &mut Criterion) {
    let mut group = c.benchmark_group("creation_and_cloning");

    // Creation (new)
    group.bench_function("new/LfrLock", |b| {
        b.iter(|| {
            let _ = LfrLock::new(Data(vec![0; 10]));
        })
    });
    group.bench_function("new/ArcSwap", |b| {
        b.iter(|| {
            let _ = ArcSwap::from_pointee(Data(vec![0; 10]));
        })
    });
    group.bench_function("new/Mutex", |b| {
        b.iter(|| {
            let _ = Mutex::new(Data(vec![0; 10]));
        })
    });

    // Cloning
    let lfr_lock = LfrLock::new(Data(vec![0; 10]));
    group.bench_function("clone/LfrLock", |b| {
        b.iter(|| {
            let _ = lfr_lock.clone();
        })
    });

    let arc_swap = Arc::new(ArcSwap::from_pointee(Data(vec![0; 10])));
    group.bench_function("clone/ArcSwap", |b| {
        b.iter(|| {
            let _ = arc_swap.clone();
        })
    });

    let mutex = Arc::new(Mutex::new(Data(vec![0; 10])));
    group.bench_function("clone/Mutex", |b| {
        b.iter(|| {
            let _ = mutex.clone();
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    read_only_single_thread,
    read_heavy_concurrent,
    write_heavy_concurrent,
    bench_creation_and_cloning
);
criterion_main!(benches);
