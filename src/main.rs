#![feature(test)]

use parking_lot::ReentrantMutex as ParkingLotMutex;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock as TokioRwLock;
use futures::lock::Mutex as FuturesMutex;
use tokio::sync::Mutex as TokioMutex;

#[tokio::main]
async fn main() {
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async move {
            let foo = ParkingLotFoo::new();
            let foo_clone = foo.clone();
            tokio::task::spawn_local(foo.bar());
            tokio::time::delay_for(Duration::from_secs(1)).await;
            foo_clone.bar().await;
        })
        .await;
}

#[derive(Clone)]
struct FuturesMutexFoo {
    foo: Arc<FuturesMutex<Vec<u32>>>,
}

impl FuturesMutexFoo {
    fn new() -> Self {
        Self {
            foo: Arc::new(FuturesMutex::new(Vec::new())),
        }
    }

    async fn write(self, val: u32) {
        let mut lck = self.foo.lock().await;
        tokio::time::delay_for(Duration::from_millis(5)).await;
        lck.push(val);
    }

    async fn read(self) -> Option<u32> {
        let mut lck = self.foo.lock().await;
        lck.get(0).cloned()
    }
}

#[derive(Clone)]
struct TokioMutexFoo {
    foo: Arc<TokioMutex<Vec<u32>>>,
}

impl TokioMutexFoo {
    fn new() -> Self {
        Self {
            foo: Arc::new(TokioMutex::new(Vec::new())),
        }
    }

    async fn write(self, val: u32) {
        let mut lck = self.foo.lock().await;
        tokio::time::delay_for(Duration::from_millis(5)).await;
        lck.push(val);
    }

    async fn read(self) -> Option<u32> {
        let mut lck = self.foo.lock().await;
        lck.get(0).cloned()
    }
}

#[derive(Clone)]
struct TokioRwLockFoo {
    foo: Arc<TokioRwLock<Vec<u32>>>,
}

impl TokioRwLockFoo {
    fn new() -> Self {
        Self {
            foo: Arc::new(TokioRwLock::new(Vec::new())),
        }
    }

    async fn bar(self) {
        self.foo.write().await.push(0u32);
        self.foo.read().await.get(0);
    }

    async fn write(self, val: u32) {
        let mut foo = self.foo.write().await;
        tokio::time::delay_for(Duration::from_millis(5)).await;
        foo.push(val);
    }

    async fn read(self) -> Option<u32> {
        let mut lck = self.foo.read().await;
        lck.get(0).cloned()
    }
}

#[derive(Clone)]
struct ParkingLotFoo {
    foo: Arc<ParkingLotMutex<RefCell<Vec<u32>>>>,
}

impl ParkingLotFoo {
    fn new() -> Self {
        Self {
            foo: Arc::new(ParkingLotMutex::new(RefCell::new(Vec::new()))),
        }
    }

    async fn bar(self) {
        let lck = self.foo.lock();
        lck.borrow_mut().push(0u32);
        lck.borrow().get(0);
    }

    async fn write(self, val: u32) {
        let foo = self.foo.lock();
        tokio::time::delay_for(Duration::from_millis(50)).await;
        foo.borrow_mut().push(val);
    }

    async fn read(self) -> Option<u32> {
        self.foo.lock().borrow().get(0).cloned()
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use test::Bencher;
    use futures::future::Either;
    use rand::seq::SliceRandom as _;
    use rand::thread_rng;

    use super::*;

    const FUTURES_COUNT: u32 = 1_000;

    fn get_sys() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new()
            .basic_scheduler()
            .threaded_scheduler()
            .enable_all()
            .build()
            .unwrap()
    }

    #[bench]
    fn tokio_bench(b: &mut Bencher) {
        let mut sys = get_sys();
        b.iter(move || {
            sys.block_on(async move {
                let foo = TokioRwLockFoo::new();
                let futs: Vec<_> = (0..FUTURES_COUNT)
                    .into_iter()
                    .map(move |_| Box::pin(foo.clone().bar()))
                    .collect();
                futures::future::select_all(futs).await;
            })
        });
    }

    #[bench]
    fn parking_lot_bench(b: &mut Bencher) {
        let mut sys = get_sys();
        b.iter(move || {
            sys.block_on(async move {
                let foo = ParkingLotFoo::new();
                let futs: Vec<_> = (0..FUTURES_COUNT)
                    .into_iter()
                    .map(move |_| Box::pin(foo.clone().bar()))
                    .collect();
                futures::future::select_all(futs).await;
            })
        });
    }

    #[bench]
    fn tokio_rwlock_write_and_read(b: &mut Bencher) {
        let mut sys = get_sys();
        b.iter(move || {
            sys.block_on(async move {
                let foo = TokioRwLockFoo::new();
                let mut futs: Vec<_> = (0..FUTURES_COUNT)
                    .into_iter()
                    .map(move |i| {
                        let foo_clone = foo.clone();
                        if i % 500 == 0 {
                            Box::pin(Either::Left(async move {
                                foo_clone.write(i).await;
                            }))
                        } else {
                            Box::pin(Either::Right(async move {
                                foo_clone.read().await;
                            }))
                        }
                    })
                    .collect();
//                futs.shuffle(&mut thread_rng());
                futures::future::select_all(futs).await;
            })
        });
    }

    #[bench]
    fn tokio_mutex_write_and_read(b: &mut Bencher) {
        let mut sys = get_sys();
        b.iter(move || {
            sys.block_on(async move {
                let foo = TokioMutexFoo::new();
                let mut futs: Vec<_> = (0..FUTURES_COUNT)
                    .into_iter()
                    .map(move |i| {
                        let foo_clone = foo.clone();
                        if i % 500 == 0 {
                            Box::pin(Either::Left(async move {
                                foo_clone.write(i).await;
                            }))
                        } else {
                            Box::pin(Either::Right(async move {
                                foo_clone.read().await;
                            }))
                        }
                    })
                    .collect();
//                futs.shuffle(&mut thread_rng());
                futures::future::select_all(futs).await;
            })
        })
    }

    #[bench]
    fn futures_mutex_write_and_read(b: &mut Bencher) {
        let mut sys = get_sys();
        b.iter(move || {
            sys.block_on(async move {
                let foo = FuturesMutexFoo::new();
                let mut futs: Vec<_> = (0..FUTURES_COUNT)
                    .into_iter()
                    .map(move |i| {
                        let foo_clone = foo.clone();
                        if i % 500 == 0 {
                            Box::pin(Either::Left(async move {
                                foo_clone.write(i).await;
                            }))
                        } else {
                            Box::pin(Either::Right(async move {
                                foo_clone.read().await;
                            }))
                        }
                    })
                    .collect();
//                futs.shuffle(&mut thread_rng());
                futures::future::select_all(futs).await;
            })
        })
    }

    /*
    #[bench]
    fn parking_lot_write_and_read(b: &mut Bencher) {
        let mut sys = get_sys();
        b.iter(move || {
            sys.block_on(async move {
                let foo = ParkingLotFoo::new();
                let futs: Vec<_> = (0..FUTURES_COUNT)
                    .into_iter()
                    .map(move |i| {
                        let foo_clone = foo.clone();
                        if i % 1000 == 0 {
                            Box::pin(Either::Left(async move {
                                foo_clone.write(i).await;
                            }))
                        } else {
                            Box::pin(Either::Right(async move {
                                foo_clone.read().await;
                            }))
                        }
                    })
                    .collect();
                futures::future::select_all(futs).await;
            })
        });
    }
    */
}
