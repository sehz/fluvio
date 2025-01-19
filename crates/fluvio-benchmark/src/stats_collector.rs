use std::{sync::Arc, time::Instant};

use anyhow::Result;
use async_channel::Sender;
use fluvio::ProduceOutput;
use fluvio_future::{sync::Mutex, task::spawn};
use hdrhistogram::Histogram;

#[derive(Debug)]
pub(crate) struct ProducerStat {
    record_send: u64,
    record_bytes: u64,
    elapsed: u128,
    min_latency: u128,
    max_latency: u128,


    //output_tx: Sender<(ProduceOutput, Instant)>,
    histogram: Arc<Mutex<Histogram<u64>>>,
}

impl ProducerStat {
    pub(crate) fn new(num_records: u64, latency_sender: Sender<Histogram<u64>>) -> Self {
        let (output_tx, rx) = async_channel::unbounded::<(ProduceOutput, Instant)>();
        let histogram = Arc::new(Mutex::new(hdrhistogram::Histogram::<u64>::new(2).unwrap()));

        ProducerStat::track_latency(num_records, latency_sender, rx, histogram.clone());

        Self {
            record_send: 0,
            record_bytes: 0,
            min_latency: 0,
            max_latency: 0,
            elapsed: 0,
          //  output_tx,
            histogram,
        }
    }

    fn track_latency(
        num_records: u64,
        latency_sender: Sender<Histogram<u64>>,
        rx: async_channel::Receiver<(ProduceOutput, Instant)>,
        histogram: Arc<Mutex<Histogram<u64>>>,
    ) {
        spawn(async move {
            while let Ok((send_out, time)) = rx.recv().await {
                let hist = histogram.clone();
                let latency_sender = latency_sender.clone();
                //spawn(async move {
                match send_out.wait().await {
                    Ok(_) => {
                        let duration = time.elapsed();
                        let mut hist = hist.lock().await;
                        hist.record(duration.as_nanos() as u64).expect("record");

                        if hist.len() >= num_records {
                            latency_sender.send(hist.clone()).await.expect("send");
                        }
                    }
                    Err(err) => {
                        println!("error sending record: {}", err);
                        return;
                    }
                }
                //});
            }
        });
    }

    /* 
    pub(crate) fn calcuate(&mut self) -> Stat {
        let elapse = self.start_time.elapsed().as_millis();
        let records_per_sec = ((self.record_send as f64 / elapse as f64) * 1000.0).round();
        let bytes_per_sec = (self.record_bytes as f64 / elapse as f64) * 1000.0;

      //  let hist = self.histogram.lock_blocking();
      //  let latency_avg = hist.mean() as u64;
      //  let latency_max = hist.value_at_quantile(1.0);

        Stat {
            records_per_sec,
            bytes_per_sec,
            _total_bytes_send: self.record_bytes,
            total_records_send: self.record_send,
         //   latency_avg,
         //   latency_max,
            _end: false,
        }
    }
    */


    //pub(crate) fn send_out(&mut self, out: (ProduceOutput, Instant)) {
    //    self.output_tx.try_send(out).expect("send out");
    //}
}

pub(crate) struct Stat {
    pub records_per_sec: f64,
    pub bytes_per_sec: f64,
    pub total_bytes_send: u64,
    pub total_records_send: u64,
    pub latency_avg: u64,
    pub latency_max: u64,
    pub latency_min: u64,
    pub _end: bool,
}

pub(crate) struct StatCollector {
    accumulator: ProducerStat,
    batch_size: u64,     // number of records before we calculate stats
    current_record: u64, // how many records we have sent in current cycle
    sender: Sender<Stat>,
}

impl StatCollector {
    pub(crate) fn create(
        batch_size: u64,
        num_records: u64,
        latency_sender: Sender<Histogram<u64>>,
        sender: Sender<Stat>,
    ) -> Self {
        Self {
            accumulator: ProducerStat::new(num_records, latency_sender),
            batch_size,
            current_record: 0,
            sender
        }
    }

    /* 
    pub(crate) fn start(&mut self) {
        self.accumulator.set_current_time();
    }
    */


    pub(crate) async fn add_record(&mut self, start: Instant,bytes: u64, send_out: ProduceOutput) -> Result<()> {
        self.accumulator.record_bytes += bytes;
        self.accumulator.record_send += 1;

        // wait for record to be sent
        send_out.wait().await?;

        let elapsed = start.elapsed().as_nanos();
        self.accumulator.elapsed += elapsed;
        self.accumulator.max_latency = self.accumulator.max_latency.max(elapsed);
        if self.accumulator.min_latency == 0 {
            self.accumulator.min_latency = elapsed;
        } else {
            self.accumulator.min_latency = self.accumulator.min_latency.min(elapsed);
        } 

        Ok(())
        
    }

    pub(crate) fn finish(&mut self) {

        println!("finish");

        let elapsed = self.accumulator.elapsed as f64;
        println!("elapsed: {} seconds",elapsed * 1e-9);
        println!("max latency: {} ms",self.accumulator.max_latency as f64 * 1e-6);
        println!("min latency: {} ms",self.accumulator.min_latency as f64 * 1e-6);
        let scale_factor = 1_000_000_000.0;
        let records_per_sec = ((self.accumulator.record_send as f64 / elapsed) * scale_factor).round();
        let bytes_per_sec = (self.accumulator.record_bytes as f64 / elapsed) * scale_factor;
        let end_record = Stat {
            records_per_sec,
            bytes_per_sec,
            total_bytes_send: self.accumulator.record_bytes,
            total_records_send: self.accumulator.record_send,
            latency_avg: 0,
            latency_max: (self.accumulator.max_latency * 1000000) as u64,
            latency_min: (self.accumulator.min_latency * 1000000) as u64,
            _end: true,
        };

        self.sender.try_send(end_record).expect("send end stats");
    }
}
